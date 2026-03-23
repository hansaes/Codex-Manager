use codexmanager_core::storage::{now_ts, Event, UsageSnapshotRecord};
use serde::Serialize;
use std::collections::HashMap;

use crate::account_availability::{evaluate_snapshot, Availability};
use crate::account_plan::{resolve_account_plan, ResolvedAccountPlan};
use crate::storage_helpers::open_storage;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteUnavailableFreeResult {
    scanned: usize,
    deleted: usize,
    skipped_available: usize,
    skipped_disabled: usize,
    skipped_non_free: usize,
    skipped_missing_usage: usize,
    skipped_missing_token: usize,
    deleted_account_ids: Vec<String>,
}

pub(crate) fn delete_unavailable_free_accounts() -> Result<DeleteUnavailableFreeResult, String> {
    let mut storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let accounts = storage.list_accounts().map_err(|err| err.to_string())?;
    let usage_by_account: HashMap<String, UsageSnapshotRecord> = storage
        .latest_usage_snapshots_by_account()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|snapshot| (snapshot.account_id.clone(), snapshot))
        .collect();

    let mut result = DeleteUnavailableFreeResult {
        scanned: 0,
        deleted: 0,
        skipped_available: 0,
        skipped_disabled: 0,
        skipped_non_free: 0,
        skipped_missing_usage: 0,
        skipped_missing_token: 0,
        deleted_account_ids: Vec::new(),
    };

    for account in accounts {
        result.scanned += 1;

        let normalized_status = account.status.trim().to_ascii_lowercase();
        if normalized_status == "disabled" {
            result.skipped_disabled += 1;
            continue;
        }

        let snapshot = usage_by_account.get(&account.id);
        if normalized_status != "unavailable" {
            let Some(snapshot) = snapshot else {
                result.skipped_missing_usage += 1;
                continue;
            };
            if matches!(evaluate_snapshot(snapshot), Availability::Available) {
                result.skipped_available += 1;
                continue;
            }
        }

        let token = storage
            .find_token_by_account_id(&account.id)
            .map_err(|err| err.to_string())?;
        let resolved_plan = resolve_account_plan(token.as_ref(), snapshot);
        let Some(plan) = resolved_plan.as_ref() else {
            if snapshot.is_none() && token.is_none() {
                result.skipped_missing_usage += 1;
            } else if token.is_none() {
                result.skipped_missing_token += 1;
            } else {
                result.skipped_non_free += 1;
            }
            continue;
        };
        if plan.normalized != "free" {
            result.skipped_non_free += 1;
            continue;
        }
        let Some(_token) = token else {
            result.skipped_missing_token += 1;
            continue;
        };

        storage
            .delete_account(&account.id)
            .map_err(|err| err.to_string())?;

        let event_message = match plan_label_for_event(resolved_plan.as_ref()) {
            Some(plan) => format!("bulk delete unavailable free account: plan={plan}"),
            None => "bulk delete unavailable free account".to_string(),
        };
        let _ = storage.insert_event(&Event {
            account_id: Some(account.id.clone()),
            event_type: "account_bulk_delete_unavailable_free".to_string(),
            message: event_message,
            created_at: now_ts(),
        });

        result.deleted += 1;
        result.deleted_account_ids.push(account.id);
    }

    Ok(result)
}

fn plan_label_for_event(plan: Option<&ResolvedAccountPlan>) -> Option<&str> {
    plan.and_then(|value| {
        if value.normalized == "unknown" {
            value.raw.as_deref()
        } else {
            Some(value.normalized.as_str())
        }
    })
}
