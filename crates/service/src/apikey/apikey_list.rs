use codexmanager_core::rpc::types::ApiKeySummary;

use crate::storage_helpers::open_storage;

pub(crate) fn read_api_keys() -> Result<Vec<ApiKeySummary>, String> {
    let storage = open_storage().ok_or_else(|| "open storage failed".to_string())?;
    let keys = storage
        .list_api_keys()
        .map_err(|err| format!("list api keys failed: {err}"))?;
    let limit_map = storage
        .list_api_key_quota_limits()
        .map_err(|err| format!("list api key quota limits failed: {err}"))?
        .into_iter()
        .map(|item| (item.key_id.clone(), item))
        .collect::<std::collections::HashMap<_, _>>();

    Ok(keys
        .into_iter()
        .map(|key| {
            let limits = limit_map.get(&key.id);
            ApiKeySummary {
                id: key.id,
                name: key.name,
                model_slug: key.model_slug,
                reasoning_effort: key.reasoning_effort,
                service_tier: key.service_tier,
                rotation_strategy: key.rotation_strategy,
                aggregate_api_id: key.aggregate_api_id,
                account_plan_filter: key.account_plan_filter,
                aggregate_api_url: key.aggregate_api_url,
                client_type: key.client_type,
                protocol_type: key.protocol_type,
                auth_scheme: key.auth_scheme,
                upstream_base_url: key.upstream_base_url,
                static_headers_json: key.static_headers_json,
                total_token_limit: limits.and_then(|item| item.total_token_limit),
                total_cost_usd_limit: limits.and_then(|item| item.total_cost_usd_limit),
                total_request_limit: limits.and_then(|item| item.total_request_limit),
                status: key.status,
                created_at: key.created_at,
                last_used_at: key.last_used_at,
            }
        })
        .collect())
}
