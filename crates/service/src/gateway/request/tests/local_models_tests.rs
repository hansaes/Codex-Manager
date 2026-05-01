use super::*;
use codexmanager_core::rpc::types::{ModelInfo, ModelsResponse};
use codexmanager_core::storage::{now_ts, AggregateApi, AggregateApiModel, ApiKey, Storage};
use serde_json::Value;

struct GlobalChannelPriorityGuard {
    enabled: bool,
    order: String,
}

impl GlobalChannelPriorityGuard {
    fn capture() -> Self {
        Self {
            enabled: crate::gateway::global_channel_priority_enabled(),
            order: crate::gateway::current_global_channel_priority_order().to_string(),
        }
    }
}

impl Drop for GlobalChannelPriorityGuard {
    fn drop(&mut self) {
        crate::gateway::set_global_channel_priority_enabled(self.enabled);
        let _ = crate::gateway::set_global_channel_priority_order(&self.order);
    }
}

/// 函数 `serialize_models_response_outputs_official_shape`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[test]
fn serialize_models_response_outputs_official_shape() {
    let items = ModelsResponse {
        models: vec![
            ModelInfo {
                slug: "gpt-5.3-codex".to_string(),
                display_name: "GPT-5.3 Codex".to_string(),
                supported_in_api: true,
                visibility: Some("list".to_string()),
                extra: [
                    ("owned_by".to_string(), Value::String("codex".to_string())),
                    (
                        "supported_endpoint_types".to_string(),
                        Value::Array(vec![Value::String("openai".to_string())]),
                    ),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            },
            ModelInfo {
                slug: "gpt-4o".to_string(),
                display_name: "GPT-4o".to_string(),
                supported_in_api: true,
                visibility: Some("list".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let output = serialize_models_response(&items);
    let value: Value = serde_json::from_str(&output).expect("valid json");
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .expect("models array");
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .expect("data array");
    assert_eq!(models.len(), 2);
    assert_eq!(data.len(), 2);
    assert_eq!(
        models[0].get("slug").and_then(Value::as_str),
        Some("gpt-5.3-codex")
    );
    assert_eq!(
        models[1].get("slug").and_then(Value::as_str),
        Some("gpt-4o")
    );
    assert_eq!(
        models[0].get("display_name").and_then(Value::as_str),
        Some("GPT-5.3 Codex")
    );
    assert_eq!(
        models[1].get("visibility").and_then(Value::as_str),
        Some("list")
    );
    assert_eq!(value.get("object").and_then(Value::as_str), Some("list"));
    assert_eq!(value.get("success").and_then(Value::as_bool), Some(true));
    assert_eq!(
        data[0].get("id").and_then(Value::as_str),
        Some("gpt-5.3-codex")
    );
    assert_eq!(data[0].get("object").and_then(Value::as_str), Some("model"));
    assert_eq!(
        data[0].get("owned_by").and_then(Value::as_str),
        Some("codex")
    );
    assert_eq!(
        data[0]
            .get("supported_endpoint_types")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("openai")
    );
}

#[test]
fn response_models_for_client_can_hide_descriptions_without_touching_metadata() {
    let items = ModelsResponse {
        models: vec![ModelInfo {
            slug: "gpt-5.3-codex".to_string(),
            display_name: "GPT-5.3 Codex".to_string(),
            description: Some("Latest frontier agentic coding model.".to_string()),
            supported_in_api: true,
            visibility: Some("list".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };

    let response = response_models_for_client(&items, true);
    assert_eq!(response.models.len(), 1);
    assert_eq!(response.models[0].slug, "gpt-5.3-codex");
    assert_eq!(response.models[0].display_name, "GPT-5.3 Codex");
    assert_eq!(response.models[0].description, None);
    assert!(response.models[0].supported_in_api);
    assert_eq!(response.models[0].visibility.as_deref(), Some("list"));

    assert_eq!(
        items.models[0].description.as_deref(),
        Some("Latest frontier agentic coding model.")
    );
}

#[test]
fn aggregate_models_response_for_key_uses_selected_aggregate_models_only() {
    let storage = Storage::open_in_memory().expect("open in memory");
    storage.init().expect("init schema");
    let now = now_ts();

    storage
        .insert_aggregate_api(&AggregateApi {
            id: "agg_models".to_string(),
            provider_type: "codex".to_string(),
            supplier_name: Some("agg".to_string()),
            sort: 0,
            url: "https://example.com/v1".to_string(),
            auth_type: "apikey".to_string(),
            auth_params_json: None,
            action: None,
            upstream_format: "responses".to_string(),
            models_path: Some("/models".to_string()),
            responses_path: None,
            chat_completions_path: None,
            proxy_mode: "follow_global".to_string(),
            proxy_url: None,
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
    storage
        .replace_aggregate_api_models(
            "agg_models",
            &[AggregateApiModel {
                aggregate_api_id: "agg_models".to_string(),
                model_slug: "deepseek-v3".to_string(),
                display_name: Some("DeepSeek-V3".to_string()),
                raw_json: "{\"id\":\"deepseek-v3\"}".to_string(),
                created_at: now,
                updated_at: now,
            }],
        )
        .expect("replace aggregate models");
    storage
        .insert_api_key(&ApiKey {
            id: "gk_agg".to_string(),
            name: Some("agg".to_string()),
            model_slug: None,
            reasoning_effort: None,
            service_tier: None,
            rotation_strategy: crate::apikey_profile::ROTATION_AGGREGATE_API.to_string(),
            aggregate_api_id: Some("agg_models".to_string()),
            account_plan_filter: None,
            aggregate_api_url: Some("https://example.com/v1".to_string()),
            client_type: "codex".to_string(),
            protocol_type: "openai_compat".to_string(),
            auth_scheme: "authorization_bearer".to_string(),
            upstream_base_url: None,
            static_headers_json: None,
            key_hash: "hash".to_string(),
            status: "active".to_string(),
            created_at: now,
            last_used_at: None,
        })
        .expect("insert api key");

    let response = aggregate_models_response_for_key(&storage, "gk_agg")
        .expect("aggregate models response")
        .expect("aggregate-specific response");
    assert_eq!(response.models.len(), 1);
    assert_eq!(response.models[0].slug, "deepseek-v3");
    assert_eq!(response.models[0].display_name, "DeepSeek-V3");
}

#[test]
fn aggregate_models_response_for_key_can_infer_single_selected_catalog_when_binding_missing() {
    let storage = Storage::open_in_memory().expect("open in memory");
    storage.init().expect("init schema");
    let now = now_ts();

    storage
        .insert_aggregate_api(&AggregateApi {
            id: "agg_infer".to_string(),
            provider_type: "codex".to_string(),
            supplier_name: Some("agg infer".to_string()),
            sort: 0,
            url: "https://example.com/v1".to_string(),
            auth_type: "apikey".to_string(),
            auth_params_json: None,
            action: None,
            upstream_format: "responses".to_string(),
            models_path: Some("/models".to_string()),
            responses_path: None,
            chat_completions_path: None,
            proxy_mode: "follow_global".to_string(),
            proxy_url: None,
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
    storage
        .replace_aggregate_api_models(
            "agg_infer",
            &[AggregateApiModel {
                aggregate_api_id: "agg_infer".to_string(),
                model_slug: "deepseek-v3".to_string(),
                display_name: Some("DeepSeek-V3".to_string()),
                raw_json: "{\"id\":\"deepseek-v3\"}".to_string(),
                created_at: now,
                updated_at: now,
            }],
        )
        .expect("replace aggregate models");
    storage
        .insert_api_key(&ApiKey {
            id: "gk_agg_infer".to_string(),
            name: Some("agg infer".to_string()),
            model_slug: None,
            reasoning_effort: None,
            service_tier: None,
            rotation_strategy: crate::apikey_profile::ROTATION_AGGREGATE_API.to_string(),
            aggregate_api_id: None,
            account_plan_filter: None,
            aggregate_api_url: None,
            client_type: "codex".to_string(),
            protocol_type: "openai_compat".to_string(),
            auth_scheme: "authorization_bearer".to_string(),
            upstream_base_url: None,
            static_headers_json: None,
            key_hash: "hash-infer".to_string(),
            status: "active".to_string(),
            created_at: now,
            last_used_at: None,
        })
        .expect("insert api key");

    let response = aggregate_models_response_for_key(&storage, "gk_agg_infer")
        .expect("aggregate models response")
        .expect("aggregate-specific response");
    assert_eq!(response.models.len(), 1);
    assert_eq!(response.models[0].slug, "deepseek-v3");
}

#[test]
fn aggregate_models_response_for_key_returns_union_of_selected_models_when_binding_missing() {
    let storage = Storage::open_in_memory().expect("open in memory");
    storage.init().expect("init schema");
    let now = now_ts();

    for (id, slug, display_name) in [
        ("agg_a", "deepseek-v3", "DeepSeek-V3"),
        ("agg_b", "gpt-5.4", "gpt-5.4"),
    ] {
        storage
            .insert_aggregate_api(&AggregateApi {
                id: id.to_string(),
                provider_type: "codex".to_string(),
                supplier_name: Some(id.to_string()),
                sort: 0,
                url: format!("https://example.com/{id}"),
                auth_type: "apikey".to_string(),
                auth_params_json: None,
                action: None,
                upstream_format: "responses".to_string(),
                models_path: Some("/models".to_string()),
                responses_path: None,
                chat_completions_path: None,
                proxy_mode: "follow_global".to_string(),
                proxy_url: None,
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
        storage
            .replace_aggregate_api_models(
                id,
                &[AggregateApiModel {
                    aggregate_api_id: id.to_string(),
                    model_slug: slug.to_string(),
                    display_name: Some(display_name.to_string()),
                    raw_json: format!("{{\"id\":\"{slug}\"}}"),
                    created_at: now,
                    updated_at: now,
                }],
            )
            .expect("replace aggregate models");
    }

    storage
        .insert_api_key(&ApiKey {
            id: "gk_agg_union".to_string(),
            name: Some("agg union".to_string()),
            model_slug: None,
            reasoning_effort: None,
            service_tier: None,
            rotation_strategy: crate::apikey_profile::ROTATION_AGGREGATE_API.to_string(),
            aggregate_api_id: None,
            account_plan_filter: None,
            aggregate_api_url: None,
            client_type: "codex".to_string(),
            protocol_type: "openai_compat".to_string(),
            auth_scheme: "authorization_bearer".to_string(),
            upstream_base_url: None,
            static_headers_json: None,
            key_hash: "hash-union".to_string(),
            status: "active".to_string(),
            created_at: now,
            last_used_at: None,
        })
        .expect("insert api key");

    let response = aggregate_models_response_for_key(&storage, "gk_agg_union")
        .expect("aggregate models response")
        .expect("aggregate-specific response");
    assert_eq!(response.models.len(), 2);
    let slugs = response
        .models
        .iter()
        .map(|item| item.slug.as_str())
        .collect::<Vec<_>>();
    assert!(slugs.contains(&"deepseek-v3"));
    assert!(slugs.contains(&"gpt-5.4"));
}

#[test]
fn aggregate_models_response_for_non_aggregate_key_uses_union_when_global_priority_enabled() {
    let _guard = crate::test_env_guard();
    let _priority_guard = GlobalChannelPriorityGuard::capture();
    crate::gateway::set_global_channel_priority_enabled(true);
    let _ = crate::gateway::set_global_channel_priority_order("account_first");

    let storage = Storage::open_in_memory().expect("open in memory");
    storage.init().expect("init schema");
    let now = now_ts();

    for (id, slug, display_name) in [
        ("agg_a", "mimo-v2.5-pro", "MIMO v2.5 Pro"),
        ("agg_b", "gpt-5.4", "gpt-5.4"),
    ] {
        storage
            .insert_aggregate_api(&AggregateApi {
                id: id.to_string(),
                provider_type: "codex".to_string(),
                supplier_name: Some(id.to_string()),
                sort: 0,
                url: format!("https://example.com/{id}"),
                auth_type: "apikey".to_string(),
                auth_params_json: None,
                action: None,
                upstream_format: "chat_completions".to_string(),
                models_path: Some("/models".to_string()),
                responses_path: None,
                chat_completions_path: None,
                proxy_mode: "follow_global".to_string(),
                proxy_url: None,
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
        storage
            .replace_aggregate_api_models(
                id,
                &[AggregateApiModel {
                    aggregate_api_id: id.to_string(),
                    model_slug: slug.to_string(),
                    display_name: Some(display_name.to_string()),
                    raw_json: format!("{{\"id\":\"{slug}\"}}"),
                    created_at: now,
                    updated_at: now,
                }],
            )
            .expect("replace aggregate models");
    }

    storage
        .insert_api_key(&ApiKey {
            id: "gk_account".to_string(),
            name: Some("account route".to_string()),
            model_slug: None,
            reasoning_effort: None,
            service_tier: None,
            rotation_strategy: crate::apikey_profile::ROTATION_ACCOUNT.to_string(),
            aggregate_api_id: None,
            account_plan_filter: None,
            aggregate_api_url: None,
            client_type: "codex".to_string(),
            protocol_type: "openai_compat".to_string(),
            auth_scheme: "authorization_bearer".to_string(),
            upstream_base_url: None,
            static_headers_json: None,
            key_hash: "hash-account".to_string(),
            status: "active".to_string(),
            created_at: now,
            last_used_at: None,
        })
        .expect("insert api key");

    let response = aggregate_models_response_for_key(&storage, "gk_account")
        .expect("aggregate models response")
        .expect("global union response");
    let slugs = response
        .models
        .iter()
        .map(|item| item.slug.as_str())
        .collect::<Vec<_>>();

    assert_eq!(response.models.len(), 2);
    assert!(slugs.contains(&"mimo-v2.5-pro"));
    assert!(slugs.contains(&"gpt-5.4"));
}
