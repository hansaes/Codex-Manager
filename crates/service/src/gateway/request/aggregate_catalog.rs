use codexmanager_core::storage::{AggregateApiModel, ApiKey, Storage};
use std::collections::HashSet;

pub(super) struct ResolvedAggregateCatalog {
    pub(super) aggregate_api_id: Option<String>,
    pub(super) models: Vec<AggregateApiModel>,
}

pub(super) fn resolve_aggregate_model_catalog(
    storage: &Storage,
    api_key: &ApiKey,
    requested_model: Option<&str>,
) -> Result<Option<ResolvedAggregateCatalog>, String> {
    if api_key.rotation_strategy != crate::apikey_profile::ROTATION_AGGREGATE_API {
        return Ok(None);
    }

    let candidates = match crate::gateway::upstream::protocol::aggregate_api::resolve_aggregate_api_rotation_candidates(
        storage,
        api_key.protocol_type.as_str(),
        None,
    ) {
        Ok(items) => items,
        Err(_) => {
            return Ok(Some(ResolvedAggregateCatalog {
                aggregate_api_id: None,
                models: Vec::new(),
            }))
        }
    };

    let mut catalogs = Vec::new();
    for candidate in candidates {
        let models = storage
            .list_aggregate_api_models(candidate.id.as_str())
            .map_err(|err| err.to_string())?;
        if !models.is_empty() {
            catalogs.push((candidate.id, models));
        }
    }

    if catalogs.len() == 1 {
        let (aggregate_api_id, models) = catalogs.remove(0);
        return Ok(Some(ResolvedAggregateCatalog {
            aggregate_api_id: Some(aggregate_api_id),
            models,
        }));
    }

    let requested_model = requested_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);

    if requested_model.is_none() {
        let mut seen = HashSet::new();
        let mut merged = Vec::new();
        for (_, models) in catalogs {
            for model in models {
                let slug = model.model_slug.trim().to_ascii_lowercase();
                if slug.is_empty() || !seen.insert(slug) {
                    continue;
                }
                merged.push(model);
            }
        }
        return Ok(Some(ResolvedAggregateCatalog {
            aggregate_api_id: None,
            models: merged,
        }));
    }

    let requested_model = requested_model.expect("checked is_some above");

    let mut matched = catalogs
        .into_iter()
        .filter(|(_, models)| {
            models.iter().any(|item| {
                item.model_slug
                    .trim()
                    .eq_ignore_ascii_case(&requested_model)
            })
        })
        .collect::<Vec<_>>();

    if matched.len() == 1 {
        let (aggregate_api_id, models) = matched.remove(0);
        Ok(Some(ResolvedAggregateCatalog {
            aggregate_api_id: Some(aggregate_api_id),
            models,
        }))
    } else {
        let mut seen = HashSet::new();
        let mut merged = Vec::new();
        for (_, models) in matched {
            for model in models {
                let slug = model.model_slug.trim().to_ascii_lowercase();
                if slug.is_empty() || !seen.insert(slug) {
                    continue;
                }
                merged.push(model);
            }
        }
        Ok(Some(ResolvedAggregateCatalog {
            aggregate_api_id: None,
            models: merged,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codexmanager_core::storage::{now_ts, AggregateApi, AggregateApiModel, ApiKey, Storage};

    fn insert_api_with_models(storage: &Storage, id: &str, sort: i64, models: &[&str]) {
        let now = now_ts();
        storage
            .insert_aggregate_api(&AggregateApi {
                id: id.to_string(),
                provider_type: "codex".to_string(),
                supplier_name: Some(id.to_string()),
                sort,
                url: format!("https://{id}.example.com/v1"),
                auth_type: "apikey".to_string(),
                auth_params_json: None,
                action: None,
                upstream_format: "responses".to_string(),
                models_path: Some("/models".to_string()),
                responses_path: None,
                chat_completions_path: None,
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
                last_test_at: None,
                last_test_status: None,
                last_test_error: None,
                models_last_synced_at: None,
                models_last_sync_status: None,
                models_last_sync_error: None,
            })
            .expect("insert aggregate api");
        let items = models
            .iter()
            .map(|slug| AggregateApiModel {
                aggregate_api_id: id.to_string(),
                model_slug: (*slug).to_string(),
                display_name: Some((*slug).to_string()),
                raw_json: format!("{{\"id\":\"{slug}\"}}"),
                created_at: now,
                updated_at: now,
            })
            .collect::<Vec<_>>();
        storage
            .replace_aggregate_api_models(id, &items)
            .expect("replace aggregate api models");
    }

    fn aggregate_key(aggregate_api_id: Option<&str>) -> ApiKey {
        ApiKey {
            id: "gk_agg".to_string(),
            name: Some("agg".to_string()),
            model_slug: None,
            reasoning_effort: None,
            service_tier: None,
            rotation_strategy: crate::apikey_profile::ROTATION_AGGREGATE_API.to_string(),
            aggregate_api_id: aggregate_api_id.map(str::to_string),
            account_plan_filter: None,
            aggregate_api_url: None,
            client_type: "codex".to_string(),
            protocol_type: "openai_compat".to_string(),
            auth_scheme: "authorization_bearer".to_string(),
            upstream_base_url: None,
            static_headers_json: None,
            key_hash: "hash".to_string(),
            status: "active".to_string(),
            created_at: now_ts(),
            last_used_at: None,
        }
    }

    #[test]
    fn requested_model_catalog_ignores_legacy_bound_api_and_merges_matching_models() {
        let storage = Storage::open_in_memory().expect("open in memory");
        storage.init().expect("init schema");
        insert_api_with_models(&storage, "agg_low", 0, &["gpt-5.4"]);
        insert_api_with_models(&storage, "agg_high", 10, &["gpt-5.4"]);
        insert_api_with_models(&storage, "agg_legacy_other", 5, &["deepseek-v3"]);

        let resolved = resolve_aggregate_model_catalog(
            &storage,
            &aggregate_key(Some("agg_legacy_other")),
            Some("gpt-5.4"),
        )
        .expect("resolve catalog")
        .expect("aggregate catalog");

        assert_eq!(resolved.aggregate_api_id, None);
        let slugs = resolved
            .models
            .iter()
            .map(|item| item.model_slug.as_str())
            .collect::<Vec<_>>();
        assert_eq!(slugs, vec!["gpt-5.4"]);
    }
}
