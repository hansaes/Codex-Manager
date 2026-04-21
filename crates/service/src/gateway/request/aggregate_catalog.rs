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

    if let Some(aggregate_api_id) = api_key
        .aggregate_api_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let models = storage
            .list_aggregate_api_models(aggregate_api_id)
            .map_err(|err| err.to_string())?;
        return Ok(Some(ResolvedAggregateCatalog {
            aggregate_api_id: Some(aggregate_api_id.to_string()),
            models,
        }));
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
            models
                .iter()
                .any(|item| item.model_slug.trim().eq_ignore_ascii_case(&requested_model))
        })
        .collect::<Vec<_>>();

    if matched.len() == 1 {
        let (aggregate_api_id, models) = matched.remove(0);
        Ok(Some(ResolvedAggregateCatalog {
            aggregate_api_id: Some(aggregate_api_id),
            models,
        }))
    } else {
        Ok(Some(ResolvedAggregateCatalog {
            aggregate_api_id: None,
            models: Vec::new(),
        }))
    }
}
