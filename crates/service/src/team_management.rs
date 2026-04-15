use chrono::{DateTime, Utc};
use codexmanager_core::auth::{
    extract_token_exp, parse_id_token_claims, DEFAULT_CLIENT_ID, DEFAULT_ISSUER,
};
use codexmanager_core::rpc::types::{
    ManagedTeamInviteResult, ManagedTeamListResult, ManagedTeamMemberSummary,
    ManagedTeamMembersResult, ManagedTeamSummary,
};
use codexmanager_core::storage::{now_ts, Account, ManagedTeam, Storage, Token};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;

use crate::storage_helpers::open_storage;
use crate::usage_token_refresh::refresh_and_persist_access_token;

const TEAM_BACKEND_BASE_URL: &str = "https://chatgpt.com/backend-api";
const TEAM_BACKEND_BASE_URL_OVERRIDE_ENV: &str = "CODEXMANAGER_TEAM_BACKEND_BASE_URL";
const TEAM_PROXY_ENV: &str = "CODEXMANAGER_UPSTREAM_PROXY_URL";
const TEAM_CLOUDFLARE_BLOCKED_MESSAGE: &str =
    "Access blocked by Cloudflare. The Team management request looks like it was challenged by chatgpt.com";
const TEAM_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
struct TeamAccountsResponse {
    #[serde(default)]
    accounts: HashMap<String, TeamAccountEnvelope>,
}

#[derive(Debug, Deserialize)]
struct TeamAccountEnvelope {
    #[serde(default)]
    account: TeamAccountInfo,
    #[serde(default)]
    entitlement: TeamEntitlement,
}

#[derive(Debug, Default, Deserialize)]
struct TeamAccountInfo {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    plan_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TeamEntitlement {
    #[serde(default)]
    subscription_plan: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedTeamAccount {
    account_id: String,
    team_name: Option<String>,
    plan_type: Option<String>,
    subscription_plan: Option<String>,
    expires_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TeamUsersResponse {
    #[serde(default)]
    items: Vec<TeamUserItem>,
    #[serde(default)]
    total: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct TeamUserItem {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    created_time: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct TeamInvitesResponse {
    #[serde(default)]
    items: Vec<TeamInviteItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct TeamInviteItem {
    #[serde(default)]
    email_address: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    created_time: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct InviteResponse {
    #[serde(default)]
    account_invites: Vec<serde_json::Value>,
}

fn backend_base_url() -> String {
    std::env::var(TEAM_BACKEND_BASE_URL_OVERRIDE_ENV)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| TEAM_BACKEND_BASE_URL.to_string())
}

fn current_team_proxy_url() -> Option<String> {
    std::env::var(TEAM_PROXY_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn system_curl_binary() -> &'static str {
    if cfg!(windows) {
        "curl.exe"
    } else {
        "curl"
    }
}

fn build_browser_like_headers(
    access_token: &str,
    team_account_id: Option<&str>,
    include_json_content_type: bool,
) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("Accept", "*/*".to_string()),
        ("Accept-Language", "en-US,en;q=0.9".to_string()),
        ("Origin", "https://chatgpt.com".to_string()),
        ("Referer", "https://chatgpt.com/".to_string()),
        ("Connection", "keep-alive".to_string()),
        ("User-Agent", TEAM_USER_AGENT.to_string()),
        ("Authorization", format!("Bearer {access_token}")),
    ];
    if let Some(team_account_id) = team_account_id {
        headers.push(("chatgpt-account-id", team_account_id.to_string()));
    }
    if include_json_content_type {
        headers.push(("Content-Type", "application/json".to_string()));
    }
    headers
}

fn run_curl_request(
    method: &str,
    url: &str,
    headers: &[(&str, String)],
    json_body: Option<&serde_json::Value>,
) -> Result<(u16, String, String), String> {
    const STATUS_MARKER: &str = "__CM_STATUS__:";
    const CONTENT_TYPE_MARKER: &str = "__CM_CONTENT_TYPE__:";

    let mut command = Command::new(system_curl_binary());
    command.arg("-sS");
    command.arg("--location");
    command.arg("--max-time").arg("30");
    command.arg("--request").arg(method);
    command.arg("--insecure");
    if let Some(proxy_url) = current_team_proxy_url() {
        command.arg("--proxy").arg(proxy_url);
    }
    for (name, value) in headers {
        command.arg("-H").arg(format!("{name}: {value}"));
    }
    if let Some(body) = json_body {
        command.arg("--data-raw").arg(body.to_string());
    }
    command.arg("--write-out").arg(format!(
        "\n{STATUS_MARKER}%{{http_code}}\n{CONTENT_TYPE_MARKER}%{{content_type}}\n"
    ));
    command.arg(url);

    let output = command.output().map_err(|err| {
        format!(
            "failed to execute {}: {}",
            system_curl_binary(),
            err
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("{} exited with status {}", system_curl_binary(), output.status)
        } else {
            stderr
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let status_idx = stdout
        .rfind(STATUS_MARKER)
        .ok_or_else(|| "curl response missing status marker".to_string())?;
    let body = stdout[..status_idx].to_string();
    let meta = &stdout[status_idx..];
    let mut status_code = 0_u16;
    let mut content_type = String::new();
    for line in meta.lines() {
        if let Some(value) = line.strip_prefix(STATUS_MARKER) {
            status_code = value.trim().parse::<u16>().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix(CONTENT_TYPE_MARKER) {
            content_type = value.trim().to_string();
        }
    }
    Ok((status_code, content_type, body))
}

fn extract_html_title(raw: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let start = lower.find("<title>")?;
    let end = lower[start + 7..].find("</title>")? + start + 7;
    let title = raw.get(start + 7..end)?.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn summarize_html_error_body(raw: &str) -> String {
    let normalized = raw.to_ascii_lowercase();
    let looks_like_blocked = normalized.contains("cloudflare") && normalized.contains("blocked");
    let looks_like_challenge = normalized.contains("cloudflare")
        || normalized.contains("just a moment")
        || normalized.contains("attention required");
    let looks_like_html = normalized.contains("<html")
        || normalized.contains("<!doctype html")
        || normalized.contains("</html>");
    if !looks_like_html {
        return raw.trim().to_string();
    }

    if looks_like_blocked {
        return TEAM_CLOUDFLARE_BLOCKED_MESSAGE.to_string();
    }

    let title = extract_html_title(raw);
    if looks_like_challenge {
        return match title {
            Some(title) => format!("Cloudflare challenge page (title={title})"),
            None => "Cloudflare challenge page".to_string(),
        };
    }

    match title {
        Some(title) => format!("upstream returned html error page (title={title})"),
        None => "upstream returned html error page".to_string(),
    }
}

fn summarize_http_error(status: reqwest::StatusCode, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return format!("team api request failed with status {status}");
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(message) = value
            .get("detail")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("message").and_then(|v| v.as_str()))
            .or_else(|| {
                value
                    .get("error")
                    .and_then(|v| v.get("message"))
                    .and_then(|v| v.as_str())
            })
        {
            return format!("team api request failed with status {status}: {message}");
        }
    }
    let html_summary = summarize_html_error_body(trimmed);
    if html_summary != trimmed {
        return format!("team api request failed with status {status}: {html_summary}");
    }
    let compact = trimmed.chars().take(240).collect::<String>();
    format!("team api request failed with status {status}: {compact}")
}

fn fetch_team_accounts(access_token: &str) -> Result<Vec<ResolvedTeamAccount>, String> {
    let url = format!("{}/accounts/check/v4-2023-04-27", backend_base_url());
    let headers = build_browser_like_headers(access_token, None, false);
    let (status_code, _content_type, body) = run_curl_request("GET", &url, &headers, None)?;
    if !(200..300).contains(&status_code) {
        let status = reqwest::StatusCode::from_u16(status_code)
            .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
        return Err(summarize_http_error(status, &body));
    }
    let payload: TeamAccountsResponse =
        serde_json::from_str(&body).map_err(|err| format!("parse team accounts json failed: {err}"))?;
    Ok(payload
        .accounts
        .into_iter()
        .filter_map(|(account_id, envelope)| {
            let plan_type = normalize_optional_text(envelope.account.plan_type);
            let subscription_plan =
                normalize_optional_text(envelope.entitlement.subscription_plan);
            if !is_team_like_plan(plan_type.as_deref())
                && !is_team_like_subscription_plan(subscription_plan.as_deref())
            {
                return None;
            }
            Some(ResolvedTeamAccount {
                account_id,
                team_name: normalize_optional_text(envelope.account.name),
                plan_type,
                subscription_plan,
                expires_at: envelope
                    .entitlement
                    .expires_at
                    .as_deref()
                    .and_then(parse_iso_to_ts),
            })
        })
        .collect())
}

fn fetch_team_users(
    access_token: &str,
    team_account_id: &str,
) -> Result<Vec<TeamUserItem>, String> {
    let mut items = Vec::new();
    let mut offset = 0_i64;
    let limit = 100_i64;

    loop {
        let url = format!(
            "{}/accounts/{}/users?limit={limit}&offset={offset}",
            backend_base_url(),
            team_account_id
        );
        let headers = build_browser_like_headers(access_token, Some(team_account_id), false);
        let (status_code, _content_type, body) = run_curl_request("GET", &url, &headers, None)?;
        if !(200..300).contains(&status_code) {
            let status = reqwest::StatusCode::from_u16(status_code)
                .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
            return Err(summarize_http_error(status, &body));
        }
        let payload: TeamUsersResponse =
            serde_json::from_str(&body).map_err(|err| format!("parse team users json failed: {err}"))?;
        let total = payload.total.unwrap_or(payload.items.len() as i64).max(0);
        items.extend(payload.items);
        if items.len() as i64 >= total || total == 0 {
            break;
        }
        offset += limit;
    }

    Ok(items)
}

fn fetch_team_invites(
    access_token: &str,
    team_account_id: &str,
) -> Result<Vec<TeamInviteItem>, String> {
    let url = format!("{}/accounts/{}/invites", backend_base_url(), team_account_id);
    let headers = build_browser_like_headers(access_token, Some(team_account_id), false);
    let (status_code, _content_type, body) = run_curl_request("GET", &url, &headers, None)?;
    if !(200..300).contains(&status_code) {
        let status = reqwest::StatusCode::from_u16(status_code)
            .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
        return Err(summarize_http_error(status, &body));
    }
    let payload: TeamInvitesResponse =
        serde_json::from_str(&body).map_err(|err| format!("parse team invites json failed: {err}"))?;
    Ok(payload.items)
}

fn send_team_invites(
    access_token: &str,
    team_account_id: &str,
    emails: &[String],
) -> Result<(), String> {
    let url = format!("{}/accounts/{}/invites", backend_base_url(), team_account_id);
    let headers = build_browser_like_headers(access_token, Some(team_account_id), true);
    let invite_payload = serde_json::json!({
        "email_addresses": emails,
        "role": "standard-user",
        "resend_emails": true,
    });
    let (status_code, _content_type, body) =
        run_curl_request("POST", &url, &headers, Some(&invite_payload))?;
    if !(200..300).contains(&status_code) {
        let status = reqwest::StatusCode::from_u16(status_code)
            .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
        return Err(summarize_http_error(status, &body));
    }
    let payload: InviteResponse =
        serde_json::from_str(&body).map_err(|err| format!("parse invite json failed: {err}"))?;
    if payload.account_invites.is_empty() {
        return Err("invite request returned success but no account_invites were created".to_string());
    }
    Ok(())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn parse_iso_to_ts(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(trimmed)
        .map(|value| value.with_timezone(&Utc).timestamp())
        .ok()
        .or_else(|| {
            DateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|value| value.with_timezone(&Utc).timestamp())
                .ok()
        })
}

fn parse_time_value(value: &Option<serde_json::Value>) -> Option<i64> {
    match value {
        Some(serde_json::Value::Number(number)) => number.as_i64(),
        Some(serde_json::Value::String(text)) => {
            text.parse::<i64>().ok().or_else(|| parse_iso_to_ts(text))
        }
        _ => None,
    }
}

fn is_team_like_plan(plan_type: Option<&str>) -> bool {
    matches!(plan_type, Some("team" | "business" | "enterprise"))
}

fn is_team_like_subscription_plan(subscription_plan: Option<&str>) -> bool {
    subscription_plan.is_some_and(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        normalized.contains("team")
            || normalized.contains("business")
            || normalized.contains("enterprise")
    })
}

fn normalize_plan_from_token(token: &Token) -> Option<String> {
    parse_id_token_claims(&token.access_token)
        .ok()
        .and_then(|claims| claims.auth.and_then(|auth| auth.chatgpt_plan_type))
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn ensure_recent_access_token(
    storage: &Storage,
    account: &Account,
    token: &mut Token,
) -> Result<(), String> {
    let exp = extract_token_exp(&token.access_token).unwrap_or(0);
    let should_refresh = exp > 0 && exp <= now_ts() + 300;
    if !should_refresh {
        return Ok(());
    }
    if token.refresh_token.trim().is_empty() {
        return Err("current account access token is expiring and no refresh token is available".to_string());
    }
    let issuer = if account.issuer.trim().is_empty() {
        DEFAULT_ISSUER
    } else {
        account.issuer.trim()
    };
    let client_id = std::env::var("CODEXMANAGER_CLIENT_ID")
        .unwrap_or_else(|_| DEFAULT_CLIENT_ID.to_string());
    refresh_and_persist_access_token(storage, token, issuer, &client_id)
}

fn pick_resolved_team_account(
    managed_team: &ManagedTeam,
    source_account: &Account,
    candidates: &[ResolvedTeamAccount],
) -> Option<ResolvedTeamAccount> {
    let preferred_ids = [
        managed_team.team_account_id.as_deref(),
        source_account.chatgpt_account_id.as_deref(),
        source_account.workspace_id.as_deref(),
    ];

    preferred_ids
        .into_iter()
        .flatten()
        .find_map(|preferred| {
            candidates
                .iter()
                .find(|candidate| candidate.account_id == preferred)
                .cloned()
        })
        .or_else(|| candidates.first().cloned())
}

fn derive_team_status(
    current_members: i64,
    pending_invites: i64,
    max_members: i64,
    expires_at: Option<i64>,
) -> String {
    if expires_at.is_some_and(|value| value <= now_ts()) {
        return "expired".to_string();
    }
    if current_members + pending_invites >= max_members.max(1) {
        return "full".to_string();
    }
    "active".to_string()
}

fn build_team_summary(
    managed_team: ManagedTeam,
    source_account: Option<&Account>,
) -> ManagedTeamSummary {
    ManagedTeamSummary {
        id: managed_team.id,
        source_account_id: managed_team.source_account_id,
        source_account_label: source_account.map(|account| account.label.clone()),
        source_account_status: source_account.map(|account| account.status.clone()),
        team_account_id: managed_team.team_account_id,
        team_name: managed_team.team_name,
        plan_type: managed_team.plan_type,
        subscription_plan: managed_team.subscription_plan,
        status: managed_team.status,
        current_members: managed_team.current_members,
        pending_invites: managed_team.pending_invites,
        max_members: managed_team.max_members,
        occupied_slots: managed_team.current_members + managed_team.pending_invites,
        expires_at: managed_team.expires_at,
        last_sync_at: managed_team.last_sync_at,
        created_at: managed_team.created_at,
        updated_at: managed_team.updated_at,
    }
}

fn load_managed_team_source(
    storage: &Storage,
    team_id: &str,
) -> Result<(ManagedTeam, Account, Token), String> {
    let managed_team = storage
        .find_managed_team_by_id(team_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "managed team not found".to_string())?;
    let source_account = storage
        .find_account_by_id(&managed_team.source_account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "source account not found".to_string())?;
    let token = storage
        .find_token_by_account_id(&managed_team.source_account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "source account token not found".to_string())?;
    Ok((managed_team, source_account, token))
}

fn sync_managed_team_internal(
    storage: &Storage,
    managed_team: ManagedTeam,
    source_account: Account,
    mut token: Token,
) -> Result<ManagedTeamSummary, String> {
    ensure_recent_access_token(storage, &source_account, &mut token)?;
    let candidates = fetch_team_accounts(&token.access_token)?;
    let resolved = pick_resolved_team_account(&managed_team, &source_account, &candidates)
        .ok_or_else(|| "no eligible Team workspace was found for this parent account".to_string())?;
    let users = fetch_team_users(&token.access_token, &resolved.account_id)?;
    let invites = fetch_team_invites(&token.access_token, &resolved.account_id)?;
    let updated = ManagedTeam {
        id: managed_team.id,
        source_account_id: managed_team.source_account_id,
        team_account_id: Some(resolved.account_id),
        team_name: resolved.team_name,
        plan_type: resolved.plan_type,
        subscription_plan: resolved.subscription_plan,
        status: derive_team_status(
            users.len() as i64,
            invites.len() as i64,
            managed_team.max_members.max(1),
            resolved.expires_at,
        ),
        current_members: users.len() as i64,
        pending_invites: invites.len() as i64,
        max_members: managed_team.max_members.max(1),
        expires_at: resolved.expires_at,
        last_sync_at: Some(now_ts()),
        created_at: managed_team.created_at,
        updated_at: now_ts(),
    };
    storage
        .insert_managed_team(&updated)
        .map_err(|err| err.to_string())?;
    Ok(build_team_summary(updated, Some(&source_account)))
}

pub(crate) fn add_managed_team_from_account(account_id: &str) -> Result<ManagedTeamSummary, String> {
    let normalized_account_id = account_id.trim();
    if normalized_account_id.is_empty() {
        return Err("accountId is required".to_string());
    }
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let source_account = storage
        .find_account_by_id(normalized_account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "source account not found".to_string())?;
    let token = storage
        .find_token_by_account_id(normalized_account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "source account token not found".to_string())?;
    let plan_type = normalize_plan_from_token(&token);
    if plan_type
        .as_deref()
        .is_some_and(|value| !is_team_like_plan(Some(value)))
    {
        return Err("only TEAM / BUSINESS / ENTERPRISE accounts can be added as parent accounts".to_string());
    }

    let existing = storage
        .find_managed_team_by_id(normalized_account_id)
        .map_err(|err| err.to_string())?;
    let managed_team = match existing {
        Some(team) => team,
        None => storage
            .create_managed_team_placeholder(normalized_account_id)
            .map_err(|err| err.to_string())?,
    };
    sync_managed_team_internal(&storage, managed_team, source_account, token)
}

pub(crate) fn list_managed_teams() -> Result<ManagedTeamListResult, String> {
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let items = storage
        .list_managed_teams()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|(team, account)| build_team_summary(team, account.as_ref()))
        .collect();
    Ok(ManagedTeamListResult { items })
}

pub(crate) fn sync_managed_team(team_id: &str) -> Result<ManagedTeamSummary, String> {
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let (managed_team, source_account, token) = load_managed_team_source(&storage, team_id)?;
    sync_managed_team_internal(&storage, managed_team, source_account, token)
}

pub(crate) fn delete_managed_team(team_id: &str) -> Result<(), String> {
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let deleted = storage
        .delete_managed_team(team_id)
        .map_err(|err| err.to_string())?;
    if !deleted {
        return Err("managed team not found".to_string());
    }
    Ok(())
}

pub(crate) fn list_managed_team_members(team_id: &str) -> Result<ManagedTeamMembersResult, String> {
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let (managed_team, source_account, mut token) = load_managed_team_source(&storage, team_id)?;
    ensure_recent_access_token(&storage, &source_account, &mut token)?;

    let team_account_id = if managed_team.team_account_id.as_deref().is_none() {
        sync_managed_team_internal(
            &storage,
            managed_team.clone(),
            source_account.clone(),
            token.clone(),
        )?
        .team_account_id
        .ok_or_else(|| "teamAccountId is missing after sync".to_string())?
    } else {
        managed_team.team_account_id.clone().unwrap_or_default()
    };

    let users = fetch_team_users(&token.access_token, &team_account_id)?;
    let invites = fetch_team_invites(&token.access_token, &team_account_id)?;
    let mut items = users
        .into_iter()
        .filter_map(|item| {
            let email = item.email.as_deref()?.trim().to_string();
            if email.is_empty() {
                return None;
            }
            let added_at = parse_time_value(&item.created_time);
            Some(ManagedTeamMemberSummary {
                email,
                name: normalize_optional_text(item.name),
                role: normalize_optional_text(item.role),
                status: "joined".to_string(),
                user_id: normalize_optional_text(item.id),
                added_at,
            })
        })
        .collect::<Vec<_>>();
    items.extend(invites.into_iter().filter_map(|item| {
        let email = item.email_address.as_deref()?.trim().to_string();
        if email.is_empty() {
            return None;
        }
        let added_at = parse_time_value(&item.created_time);
        Some(ManagedTeamMemberSummary {
            email,
            name: None,
            role: normalize_optional_text(item.role),
            status: "invited".to_string(),
            user_id: None,
            added_at,
        })
    }));
    items.sort_by(|left, right| left.email.cmp(&right.email));

    Ok(ManagedTeamMembersResult {
        team_id: team_id.to_string(),
        items,
    })
}

pub(crate) fn invite_managed_team_members(
    team_id: &str,
    emails: Vec<String>,
) -> Result<ManagedTeamInviteResult, String> {
    let normalized_team_id = team_id.trim();
    if normalized_team_id.is_empty() {
        return Err("teamId is required".to_string());
    }
    let normalized_emails = emails
        .into_iter()
        .flat_map(|item| {
            item.split([',', '\n', '\r'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .filter(|value| value.contains('@'))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if normalized_emails.is_empty() {
        return Err("at least one valid email is required".to_string());
    }

    let synced = sync_managed_team(normalized_team_id)?;
    if synced.status == "expired" {
        return Err("the parent account Team has expired".to_string());
    }
    if synced.occupied_slots >= synced.max_members {
        return Err("the parent account Team is already full".to_string());
    }

    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let (managed_team, source_account, mut token) =
        load_managed_team_source(&storage, normalized_team_id)?;
    ensure_recent_access_token(&storage, &source_account, &mut token)?;
    let team_account_id = managed_team
        .team_account_id
        .clone()
        .or_else(|| synced.team_account_id.clone())
        .ok_or_else(|| "teamAccountId is missing; sync the parent account first".to_string())?;

    send_team_invites(&token.access_token, &team_account_id, &normalized_emails)?;

    let members = list_managed_team_members(normalized_team_id)?;
    let live_emails = members
        .items
        .iter()
        .map(|item| item.email.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let missing = normalized_emails
        .iter()
        .filter(|email| !live_emails.contains(email.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "invite request returned success, but these emails were not found in the team roster afterward: {}",
            missing.join(", ")
        ));
    }

    let synced_after = sync_managed_team(normalized_team_id)?;
    Ok(ManagedTeamInviteResult {
        invited_count: normalized_emails.len() as i64,
        team_id: synced_after.id,
        message: format!("invited {} member(s)", normalized_emails.len()),
    })
}
