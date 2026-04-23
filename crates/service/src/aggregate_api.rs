use codexmanager_core::rpc::types::{
    AggregateApiCreateResult, AggregateApiFetchModelsResult, AggregateApiFetchedModelSummary,
    AggregateApiModelSummary, AggregateApiSaveModelsResult, AggregateApiSecretResult,
    AggregateApiSummary, AggregateApiTestResult,
};
use codexmanager_core::storage::{now_ts, AggregateApi, AggregateApiModel};
use reqwest::header::{HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::io::Read;
use std::time::Instant;

use crate::apikey_profile::normalize_upstream_base_url;
use crate::gateway;
use crate::storage_helpers::{generate_aggregate_api_id, open_storage};

pub(crate) const AGGREGATE_API_PROVIDER_CODEX: &str = "codex";
pub(crate) const AGGREGATE_API_PROVIDER_CLAUDE: &str = "claude";
pub(crate) const AGGREGATE_API_AUTH_APIKEY: &str = "apikey";
pub(crate) const AGGREGATE_API_AUTH_USERPASS: &str = "userpass";
pub(crate) const AGGREGATE_API_REQUEST_MODELS_PATH: &str = "/v1/models";
pub(crate) const AGGREGATE_API_REQUEST_RESPONSES_PATH: &str = "/v1/responses";
pub(crate) const AGGREGATE_API_REQUEST_CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";
pub(crate) const AGGREGATE_API_PROXY_MODE_FOLLOW_GLOBAL: &str = "follow_global";
pub(crate) const AGGREGATE_API_PROXY_MODE_DIRECT: &str = "direct";
pub(crate) const AGGREGATE_API_PROXY_MODE_CUSTOM: &str = "custom";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AggregateApiProxyStrategy {
    FollowGlobal,
    Direct,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AggregateApiEndpointKind {
    Models,
    Responses,
    ChatCompletions,
    Other,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserPassSecret {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyAuthParams {
    location: String,
    name: String,
    #[serde(default)]
    header_value_format: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserPassAuthParams {
    mode: String,
    #[serde(default)]
    username_name: Option<String>,
    #[serde(default)]
    password_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AggregateApiModelSelectionInput {
    model_slug: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    raw_json: Option<String>,
}

/// 函数 `normalize_secret`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
///
/// # 返回
/// 返回函数执行结果
fn normalize_secret(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// 函数 `normalize_supplier_name`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
///
/// # 返回
/// 返回函数执行结果
fn normalize_supplier_name(value: Option<String>) -> Result<String, String> {
    let normalized = value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "supplier name is required".to_string())?;
    Ok(normalized)
}

/// 函数 `normalize_sort`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
///
/// # 返回
/// 返回函数执行结果
fn normalize_sort(value: Option<i64>) -> i64 {
    value.unwrap_or(0)
}

fn normalize_status(value: Option<String>) -> Result<String, String> {
    match value {
        Some(raw) => {
            let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
            match normalized.as_str() {
                "active" | "enabled" | "enable" => Ok("active".to_string()),
                "disabled" | "disable" | "inactive" => Ok("disabled".to_string()),
                other => Err(format!("unsupported aggregate api status: {other}")),
            }
        }
        None => Ok("active".to_string()),
    }
}

fn normalize_auth_type(value: Option<String>) -> Result<String, String> {
    match value {
        Some(raw) => {
            let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
            match normalized.as_str() {
                "apikey" | "api_key" | "key" => Ok(AGGREGATE_API_AUTH_APIKEY.to_string()),
                "userpass" | "username_password" | "account_password" | "basic" | "http_basic" => {
                    Ok(AGGREGATE_API_AUTH_USERPASS.to_string())
                }
                other => Err(format!("unsupported aggregate api auth type: {other}")),
            }
        }
        None => Ok(AGGREGATE_API_AUTH_APIKEY.to_string()),
    }
}

fn normalize_action(value: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let normalized = trimmed.to_string();
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Err("aggregate api action must be a path, not a full url".to_string());
    }
    if normalized.contains("://") {
        return Err("aggregate api action is invalid".to_string());
    }
    let with_slash = if normalized.starts_with('/') {
        normalized
    } else {
        format!("/{normalized}")
    };
    Ok(Some(with_slash))
}

fn normalize_upstream_format(value: Option<String>) -> Result<String, String> {
    match value {
        Some(raw) => {
            let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
            match normalized.as_str() {
                "responses" | "response" => Ok("responses".to_string()),
                "chat_completions" | "chat/completions" | "chat_completion" => {
                    Ok("chat_completions".to_string())
                }
                other => Err(format!(
                    "unsupported aggregate api upstream format: {other}"
                )),
            }
        }
        None => Ok("responses".to_string()),
    }
}

fn normalize_proxy_mode(value: Option<String>) -> Result<String, String> {
    match value {
        Some(raw) => {
            let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
            match normalized.as_str() {
                "" | "follow_global" | "global" | "default" => {
                    Ok(AGGREGATE_API_PROXY_MODE_FOLLOW_GLOBAL.to_string())
                }
                "direct" | "none" | "no_proxy" => Ok(AGGREGATE_API_PROXY_MODE_DIRECT.to_string()),
                "custom" | "custom_proxy" => Ok(AGGREGATE_API_PROXY_MODE_CUSTOM.to_string()),
                other => Err(format!("unsupported aggregate api proxy mode: {other}")),
            }
        }
        None => Ok(AGGREGATE_API_PROXY_MODE_FOLLOW_GLOBAL.to_string()),
    }
}

fn rewrite_proxy_url(proxy_url: &str) -> String {
    let trimmed = proxy_url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("https://socks5://") {
        return format!("socks5://{}", &trimmed["https://socks5://".len()..]);
    }
    if lower.starts_with("http://socks5://") {
        return format!("socks5://{}", &trimmed["http://socks5://".len()..]);
    }
    trimmed.to_string()
}

fn normalize_proxy_url(value: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let normalized = rewrite_proxy_url(trimmed);
    reqwest::Proxy::all(normalized.as_str())
        .map_err(|err| format!("invalid aggregate api proxy url: {err}"))?;
    Ok(Some(normalized))
}

fn normalize_models_path(value: Option<String>) -> Result<Option<String>, String> {
    normalize_endpoint_path(value, "models")
}

fn normalize_responses_path(value: Option<String>) -> Result<Option<String>, String> {
    normalize_endpoint_path(value, "responses")
}

fn normalize_chat_completions_path(value: Option<String>) -> Result<Option<String>, String> {
    normalize_endpoint_path(value, "chat completions")
}

fn normalize_endpoint_path(value: Option<String>, label: &str) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.contains("://")
    {
        return Err(format!(
            "aggregate api {label} path must be a path, not a full url"
        ));
    }
    if trimmed.starts_with('/') {
        Ok(Some(trimmed.to_string()))
    } else {
        Ok(Some(format!("/{trimmed}")))
    }
}

fn resolve_aggregate_api_proxy_strategy(
    api: &AggregateApi,
) -> Result<AggregateApiProxyStrategy, String> {
    let proxy_mode = normalize_proxy_mode(Some(api.proxy_mode.clone()))?;
    match proxy_mode.as_str() {
        AGGREGATE_API_PROXY_MODE_FOLLOW_GLOBAL => Ok(AggregateApiProxyStrategy::FollowGlobal),
        AGGREGATE_API_PROXY_MODE_DIRECT => Ok(AggregateApiProxyStrategy::Direct),
        AGGREGATE_API_PROXY_MODE_CUSTOM => {
            let proxy_url = normalize_proxy_url(api.proxy_url.clone())?
                .ok_or_else(|| "aggregate api custom proxy url is required".to_string())?;
            Ok(AggregateApiProxyStrategy::Custom(proxy_url))
        }
        _ => Err("unsupported aggregate api proxy mode".to_string()),
    }
}

pub(crate) fn build_aggregate_api_client(
    api: &AggregateApi,
) -> Result<reqwest::blocking::Client, String> {
    match resolve_aggregate_api_proxy_strategy(api)? {
        AggregateApiProxyStrategy::FollowGlobal => Ok(gateway::fresh_upstream_client()),
        AggregateApiProxyStrategy::Direct => Ok(gateway::fresh_upstream_client_without_proxy()),
        AggregateApiProxyStrategy::Custom(proxy_url) => Ok(
            gateway::fresh_upstream_client_with_proxy_override(Some(proxy_url.as_str())),
        ),
    }
}

fn normalize_auth_params_json(
    auth_type: &str,
    enabled: Option<bool>,
    auth_params: Option<serde_json::Value>,
) -> Result<Option<String>, String> {
    match enabled {
        None => Ok(None),
        Some(false) => Ok(Some(String::new())),
        Some(true) => {
            let value = auth_params.ok_or_else(|| "authParams is required".to_string())?;
            let obj = value
                .as_object()
                .ok_or_else(|| "authParams must be a JSON object".to_string())?;
            if obj.is_empty() {
                return Err("authParams must not be empty".to_string());
            }
            if auth_type == AGGREGATE_API_AUTH_APIKEY {
                let parsed: ApiKeyAuthParams = serde_json::from_value(value.clone())
                    .map_err(|_| "authParams is invalid".to_string())?;
                let location = parsed.location.trim().to_ascii_lowercase();
                if location != "header" && location != "query" {
                    return Err("authParams.location must be header or query".to_string());
                }
                if parsed.name.trim().is_empty() {
                    return Err("authParams.name is required".to_string());
                }
                if location == "header" {
                    let format = parsed
                        .header_value_format
                        .as_deref()
                        .unwrap_or("bearer")
                        .trim()
                        .to_ascii_lowercase();
                    if format != "bearer" && format != "raw" {
                        return Err(
                            "authParams.headerValueFormat must be bearer or raw".to_string()
                        );
                    }
                }
            } else if auth_type == AGGREGATE_API_AUTH_USERPASS {
                let parsed: UserPassAuthParams = serde_json::from_value(value.clone())
                    .map_err(|_| "authParams is invalid".to_string())?;
                let mode = parsed.mode.trim().to_ascii_lowercase();
                match mode.as_str() {
                    "basic" => {}
                    "headerpair" | "querypair" => {
                        if parsed
                            .username_name
                            .as_deref()
                            .map(str::trim)
                            .unwrap_or("")
                            .is_empty()
                        {
                            return Err("authParams.usernameName is required".to_string());
                        }
                        if parsed
                            .password_name
                            .as_deref()
                            .map(str::trim)
                            .unwrap_or("")
                            .is_empty()
                        {
                            return Err("authParams.passwordName is required".to_string());
                        }
                    }
                    _ => {
                        return Err(
                            "authParams.mode must be basic, headerPair, or queryPair".to_string()
                        );
                    }
                }
            }
            serde_json::to_string(&value)
                .map(Some)
                .map_err(|_| "authParams must be a valid JSON object".to_string())
        }
    }
}

fn normalize_action_override(
    enabled: Option<bool>,
    action: Option<String>,
) -> Result<Option<Option<String>>, String> {
    match enabled {
        None => Ok(None),
        Some(false) => Ok(Some(None)),
        Some(true) => normalize_action(action).map(|value| Some(Some(value.unwrap_or_default()))),
    }
}

#[cfg(test)]
mod tests {
    use codexmanager_core::storage::{AggregateApi, AggregateApiModel};

    use super::{
        action_path_or_default, legacy_root_models_retry_url, normalize_action_override,
        normalize_aggregate_api_models_payload, normalize_models_path, normalize_proxy_mode,
        normalize_proxy_url, normalize_selected_aggregate_api_models, normalize_upstream_format,
        preferred_codex_probe_model, resolve_aggregate_api_proxy_strategy,
        resolve_aggregate_api_request_path,
    };

    fn aggregate_api_with_action(action: Option<&str>) -> AggregateApi {
        AggregateApi {
            id: "agg-test".to_string(),
            provider_type: "claude".to_string(),
            supplier_name: Some("test".to_string()),
            sort: 0,
            url: "https://open.bigmodel.cn/api/anthropic".to_string(),
            auth_type: "apikey".to_string(),
            auth_params_json: None,
            action: action.map(str::to_string),
            upstream_format: "responses".to_string(),
            models_path: Some("/models".to_string()),
            responses_path: None,
            chat_completions_path: None,
            proxy_mode: "follow_global".to_string(),
            proxy_url: None,
            status: "active".to_string(),
            created_at: 0,
            updated_at: 0,
            last_test_at: None,
            last_test_status: None,
            last_test_error: None,
            models_last_synced_at: None,
            models_last_sync_status: None,
            models_last_sync_error: None,
        }
    }

    #[test]
    fn action_override_disabled_stays_none() {
        let value =
            normalize_action_override(Some(false), Some("/v1/messages".to_string())).unwrap();
        assert_eq!(value, Some(None));
    }

    #[test]
    fn action_override_enabled_and_empty_preserves_empty_string() {
        let value = normalize_action_override(Some(true), Some("   ".to_string())).unwrap();
        assert_eq!(value, Some(Some(String::new())));
    }

    #[test]
    fn empty_action_uses_default_path() {
        let api = aggregate_api_with_action(Some(""));
        let path = action_path_or_default(&api, "/v1/messages?beta=true");
        assert_eq!(path, "/v1/messages?beta=true");
    }

    #[test]
    fn normalize_upstream_format_accepts_chat_completions_aliases() {
        assert_eq!(
            normalize_upstream_format(Some("chat/completions".to_string())).unwrap(),
            "chat_completions"
        );
        assert_eq!(
            normalize_upstream_format(Some("responses".to_string())).unwrap(),
            "responses"
        );
    }

    #[test]
    fn normalize_models_path_rejects_full_url() {
        let err = normalize_models_path(Some("https://example.com/models".to_string()))
            .expect_err("full url should be rejected");
        assert!(err.contains("path"));
    }

    #[test]
    fn normalize_proxy_mode_defaults_to_follow_global() {
        assert_eq!(
            normalize_proxy_mode(None).unwrap(),
            "follow_global".to_string()
        );
        assert_eq!(
            normalize_proxy_mode(Some("global".to_string())).unwrap(),
            "follow_global".to_string()
        );
    }

    #[test]
    fn normalize_proxy_url_rejects_invalid_values() {
        let err = normalize_proxy_url(Some("not a proxy".to_string()))
            .expect_err("invalid proxy should be rejected");
        assert!(err.contains("proxy url"));
    }

    #[test]
    fn resolve_proxy_strategy_prefers_api_override() {
        let mut api = aggregate_api_with_action(None);
        api.proxy_mode = "direct".to_string();
        assert_eq!(
            resolve_aggregate_api_proxy_strategy(&api).unwrap(),
            super::AggregateApiProxyStrategy::Direct
        );

        api.proxy_mode = "custom".to_string();
        api.proxy_url = Some("http://127.0.0.1:7890".to_string());
        assert_eq!(
            resolve_aggregate_api_proxy_strategy(&api).unwrap(),
            super::AggregateApiProxyStrategy::Custom("http://127.0.0.1:7890".to_string())
        );
    }

    #[test]
    fn normalize_aggregate_api_models_payload_reads_openai_models_shape() {
        let models = normalize_aggregate_api_models_payload(&serde_json::json!({
            "data": [
                { "id": "gpt-4.1", "owned_by": "openai" },
                { "id": "gpt-4.1-mini", "object": "model" }
            ]
        }))
        .expect("normalize models");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].model_slug, "gpt-4.1");
        assert_eq!(models[0].display_name.as_deref(), Some("gpt-4.1"));
    }

    #[test]
    fn normalize_selected_aggregate_models_dedupes_and_fills_defaults() {
        let models = normalize_selected_aggregate_api_models(
            "agg-test",
            &serde_json::json!([
                { "modelSlug": "gpt-4.1", "displayName": "GPT-4.1", "rawJson": "{\"id\":\"gpt-4.1\"}" },
                { "modelSlug": "gpt-4.1", "displayName": "Duplicate" },
                { "modelSlug": "gpt-4.1-mini" }
            ]),
        )
        .expect("normalize selected models");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].aggregate_api_id, "agg-test");
        assert_eq!(models[0].model_slug, "gpt-4.1");
        assert_eq!(models[1].display_name.as_deref(), Some("gpt-4.1-mini"));
        assert!(models[1].raw_json.contains("gpt-4.1-mini"));
    }

    #[test]
    fn configured_models_path_keeps_base_v1_prefix() {
        let mut api = aggregate_api_with_action(None);
        api.url = "https://api.1l1l1l1.xyz/v1".to_string();
        api.models_path = Some("/models".to_string());

        let path = resolve_aggregate_api_request_path(&api, "/v1/models");

        assert_eq!(path, "/v1/models");
    }

    #[test]
    fn configured_responses_path_keeps_custom_base_prefix() {
        let mut api = aggregate_api_with_action(None);
        api.url = "https://gpt.mirbuds.com/codex".to_string();
        api.responses_path = Some("/responses".to_string());

        let path = resolve_aggregate_api_request_path(&api, "/v1/responses");

        assert_eq!(path, "/codex/responses");
    }

    #[test]
    fn root_base_legacy_models_path_upgrades_to_v1_models() {
        let mut api = aggregate_api_with_action(None);
        api.url = "https://hub.oaifree.com".to_string();
        api.models_path = Some("/models".to_string());
        let current_url = "https://hub.oaifree.com/models?client_version=1.0.0".to_string();

        let retry_url = legacy_root_models_retry_url(&api, current_url.as_str());

        assert_eq!(
            retry_url.as_deref(),
            Some("https://hub.oaifree.com/v1/models")
        );
    }

    #[test]
    fn preferred_codex_probe_model_uses_first_imported_model() {
        let models = vec![
            AggregateApiModel {
                aggregate_api_id: "agg-test".to_string(),
                model_slug: "   ".to_string(),
                display_name: None,
                raw_json: "{}".to_string(),
                created_at: 0,
                updated_at: 0,
            },
            AggregateApiModel {
                aggregate_api_id: "agg-test".to_string(),
                model_slug: "gpt-5.4".to_string(),
                display_name: Some("GPT 5.4".to_string()),
                raw_json: "{}".to_string(),
                created_at: 0,
                updated_at: 0,
            },
        ];

        assert_eq!(
            preferred_codex_probe_model(&models).as_deref(),
            Some("gpt-5.4")
        );
    }
}

fn split_request_path(path: &str) -> (&str, Option<&str>) {
    let trimmed = path.trim();
    trimmed
        .split_once('?')
        .map_or((trimmed, None), |(path, query)| (path, Some(query)))
}

fn normalize_path_part(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let normalized = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    if normalized.len() > 1 {
        normalized.trim_end_matches('/').to_string()
    } else {
        normalized
    }
}

fn join_path_segments(prefix: &str, suffix: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    let suffix = suffix.trim_start_matches('/');
    if prefix.is_empty() {
        if suffix.is_empty() {
            "/".to_string()
        } else {
            format!("/{suffix}")
        }
    } else if suffix.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}/{suffix}")
    }
}

fn request_path_for_prefix(request_path: &str, include_v1: bool) -> String {
    let normalized = normalize_path_part(request_path);
    if include_v1 {
        return normalized;
    }
    if let Some(rest) = normalized.strip_prefix("/v1") {
        if rest.is_empty() {
            "/".to_string()
        } else {
            rest.to_string()
        }
    } else {
        normalized
    }
}

fn attach_query(path: String, query: Option<&str>) -> String {
    match query.map(str::trim).filter(|value| !value.is_empty()) {
        Some(query) => format!("{path}?{query}"),
        None => path,
    }
}

fn fixed_endpoint_template(base_path: &str) -> Option<(String, bool)> {
    let normalized = normalize_path_part(base_path);
    let lower = normalized.to_ascii_lowercase();
    for (suffix, include_v1) in [
        ("/v1/chat/completions", true),
        ("/chat/completions", false),
        ("/v1/responses", true),
        ("/responses", false),
        ("/v1/models", true),
        ("/models", false),
    ] {
        if lower == suffix || lower.ends_with(suffix) {
            let prefix_len = normalized.len() - suffix.len();
            let prefix = normalized[..prefix_len].to_string();
            return Some((prefix, include_v1));
        }
    }
    None
}

fn aggregate_api_endpoint_kind(path: &str) -> AggregateApiEndpointKind {
    match normalize_path_part(path).to_ascii_lowercase().as_str() {
        "/v1/models" | "/models" => AggregateApiEndpointKind::Models,
        "/v1/responses" | "/responses" => AggregateApiEndpointKind::Responses,
        "/v1/chat/completions" | "/chat/completions" => AggregateApiEndpointKind::ChatCompletions,
        _ => AggregateApiEndpointKind::Other,
    }
}

fn aggregate_api_endpoint_request_path(kind: AggregateApiEndpointKind) -> Option<&'static str> {
    match kind {
        AggregateApiEndpointKind::Models => Some(AGGREGATE_API_REQUEST_MODELS_PATH),
        AggregateApiEndpointKind::Responses => Some(AGGREGATE_API_REQUEST_RESPONSES_PATH),
        AggregateApiEndpointKind::ChatCompletions => {
            Some(AGGREGATE_API_REQUEST_CHAT_COMPLETIONS_PATH)
        }
        AggregateApiEndpointKind::Other => None,
    }
}

fn aggregate_api_endpoint_suffix(
    kind: AggregateApiEndpointKind,
    include_v1: bool,
) -> Option<&'static str> {
    match (kind, include_v1) {
        (AggregateApiEndpointKind::Models, true) => Some("/v1/models"),
        (AggregateApiEndpointKind::Models, false) => Some("/models"),
        (AggregateApiEndpointKind::Responses, true) => Some("/v1/responses"),
        (AggregateApiEndpointKind::Responses, false) => Some("/responses"),
        (AggregateApiEndpointKind::ChatCompletions, true) => Some("/v1/chat/completions"),
        (AggregateApiEndpointKind::ChatCompletions, false) => Some("/chat/completions"),
        (AggregateApiEndpointKind::Other, _) => None,
    }
}

fn normalized_legacy_action_path(api: &AggregateApi) -> Option<String> {
    api.action
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.starts_with('/') {
                value.to_string()
            } else {
                format!("/{value}")
            }
        })
}

fn configured_endpoint_path(api: &AggregateApi, kind: AggregateApiEndpointKind) -> Option<String> {
    match kind {
        AggregateApiEndpointKind::Models => api
            .models_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_path_part),
        AggregateApiEndpointKind::Responses => api
            .responses_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_path_part)
            .or_else(|| normalized_legacy_action_path(api)),
        AggregateApiEndpointKind::ChatCompletions => api
            .chat_completions_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_path_part)
            .or_else(|| normalized_legacy_action_path(api)),
        AggregateApiEndpointKind::Other => normalized_legacy_action_path(api),
    }
}

fn auto_resolve_aggregate_api_path(
    base_url: &str,
    request_path: &str,
    kind: AggregateApiEndpointKind,
) -> String {
    let (path_part, query_part) = split_request_path(request_path);
    let normalized_request = normalize_path_part(path_part);
    let base_path = reqwest::Url::parse(base_url)
        .map(|url| normalize_path_part(url.path()))
        .unwrap_or_else(|_| "/".to_string());

    let resolved = if let Some((prefix, include_v1)) = fixed_endpoint_template(base_path.as_str()) {
        if let Some(suffix) = aggregate_api_endpoint_suffix(kind, include_v1) {
            join_path_segments(prefix.as_str(), suffix)
        } else {
            join_path_segments(
                prefix.as_str(),
                request_path_for_prefix(normalized_request.as_str(), include_v1).as_str(),
            )
        }
    } else if base_path == "/" {
        aggregate_api_endpoint_suffix(kind, true)
            .map(str::to_string)
            .unwrap_or(normalized_request)
    } else if base_path.to_ascii_lowercase().ends_with("/v1") {
        if let Some(suffix) = aggregate_api_endpoint_suffix(kind, false) {
            join_path_segments(base_path.as_str(), suffix)
        } else {
            join_path_segments(
                base_path.as_str(),
                request_path_for_prefix(normalized_request.as_str(), false).as_str(),
            )
        }
    } else if let Some(suffix) = aggregate_api_endpoint_suffix(kind, false) {
        join_path_segments(base_path.as_str(), suffix)
    } else {
        join_path_segments(base_path.as_str(), normalized_request.as_str())
    };

    attach_query(resolved, query_part)
}

fn resolve_configured_aggregate_api_path(
    base_url: &str,
    configured_path: &str,
    fallback_query: Option<&str>,
) -> String {
    let (configured_path_part, configured_query) = split_request_path(configured_path);
    let normalized_configured = normalize_path_part(configured_path_part);
    let configured_kind = aggregate_api_endpoint_kind(normalized_configured.as_str());
    let base_path = reqwest::Url::parse(base_url)
        .map(|url| normalize_path_part(url.path()))
        .unwrap_or_else(|_| "/".to_string());

    let resolved = if configured_kind == AggregateApiEndpointKind::Other {
        normalized_configured
    } else if let Some((prefix, include_v1)) = fixed_endpoint_template(base_path.as_str()) {
        aggregate_api_endpoint_suffix(configured_kind, include_v1)
            .map(|suffix| join_path_segments(prefix.as_str(), suffix))
            .unwrap_or_else(|| {
                join_path_segments(
                    prefix.as_str(),
                    request_path_for_prefix(normalized_configured.as_str(), include_v1).as_str(),
                )
            })
    } else if base_path == "/" {
        normalized_configured
    } else {
        let base_lower = base_path.to_ascii_lowercase();
        let configured_lower = normalized_configured.to_ascii_lowercase();
        if base_lower.ends_with("/v1")
            && (configured_lower == "/v1" || configured_lower.starts_with("/v1/"))
        {
            let base_prefix = base_path
                .strip_suffix("/v1")
                .filter(|value| !value.is_empty())
                .unwrap_or("/");
            join_path_segments(base_prefix, normalized_configured.as_str())
        } else {
            join_path_segments(base_path.as_str(), normalized_configured.as_str())
        }
    };

    attach_query(resolved, configured_query.or(fallback_query))
}

pub(crate) fn resolve_aggregate_api_request_path(api: &AggregateApi, request_path: &str) -> String {
    let kind = aggregate_api_endpoint_kind(split_request_path(request_path).0);
    if let Some(configured_path) = configured_endpoint_path(api, kind) {
        return resolve_configured_aggregate_api_path(
            api.url.as_str(),
            configured_path.as_str(),
            split_request_path(request_path).1,
        );
    }
    auto_resolve_aggregate_api_path(api.url.as_str(), request_path, kind)
}

fn build_aggregate_api_request_url(
    api: &AggregateApi,
    request_path: &str,
) -> Result<String, String> {
    let mut url = reqwest::Url::parse(api.url.as_str())
        .map_err(|_| "invalid aggregate api url".to_string())?;
    let resolved_path = resolve_aggregate_api_request_path(api, request_path);
    let (path_part, query_part) = split_request_path(resolved_path.as_str());
    url.set_path(path_part);
    url.set_query(query_part.filter(|value| !value.trim().is_empty()));
    Ok(url.to_string())
}

fn build_aggregate_api_endpoint_url(
    api: &AggregateApi,
    kind: AggregateApiEndpointKind,
) -> Result<String, String> {
    let request_path = aggregate_api_endpoint_request_path(kind)
        .ok_or_else(|| "unsupported aggregate api endpoint kind".to_string())?;
    build_aggregate_api_request_url(api, request_path)
}

fn serialize_userpass_secret(username: &str, password: &str) -> Result<String, String> {
    let secret = UserPassSecret {
        username: username.trim().to_string(),
        password: password.trim().to_string(),
    };
    serde_json::to_string(&secret).map_err(|_| "invalid username/password".to_string())
}

fn action_path_or_default(api: &AggregateApi, default: &str) -> String {
    match api.action.as_deref().map(str::trim) {
        Some("") => default.to_string(),
        Some(value) => {
            if value.starts_with('/') {
                value.to_string()
            } else {
                format!("/{value}")
            }
        }
        None => default.to_string(),
    }
}

fn with_query_param(url: &str, name: &str, value: &str) -> String {
    let mut parsed = match reqwest::Url::parse(url) {
        Ok(value) => value,
        Err(_) => return url.to_string(),
    };
    let existing = parsed.query_pairs().into_owned().collect::<Vec<_>>();
    parsed.set_query(None);
    {
        let mut query = parsed.query_pairs_mut();
        for (key, val) in existing {
            if key == name {
                continue;
            }
            query.append_pair(key.as_str(), val.as_str());
        }
        query.append_pair(name, value);
    }
    parsed.to_string()
}

fn apply_probe_auth(
    mut builder: reqwest::blocking::RequestBuilder,
    mut url: String,
    api: &AggregateApi,
    secret: &str,
) -> Result<(reqwest::blocking::RequestBuilder, String), String> {
    let auth_type = normalize_auth_type(Some(api.auth_type.clone()))?;
    let auth_params = api
        .auth_params_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if auth_type == AGGREGATE_API_AUTH_USERPASS {
        let parsed: UserPassSecret = serde_json::from_str(secret.trim())
            .map_err(|_| "invalid aggregate api secret".to_string())?;
        if let Some(raw) = auth_params {
            let params: UserPassAuthParams =
                serde_json::from_str(raw).map_err(|_| "invalid authParams".to_string())?;
            let mode = params.mode.trim().to_ascii_lowercase();
            if mode == "headerpair" {
                let username_name = params.username_name.as_deref().unwrap_or("username").trim();
                let password_name = params.password_name.as_deref().unwrap_or("password").trim();
                builder = builder
                    .header(username_name, parsed.username.as_str())
                    .header(password_name, parsed.password.as_str());
                return Ok((builder, url));
            }
            if mode == "querypair" {
                let username_name = params.username_name.as_deref().unwrap_or("username").trim();
                let password_name = params.password_name.as_deref().unwrap_or("password").trim();
                url = with_query_param(url.as_str(), username_name, parsed.username.as_str());
                url = with_query_param(url.as_str(), password_name, parsed.password.as_str());
                return Ok((builder, url));
            }
        }
        builder = builder.basic_auth(parsed.username, Some(parsed.password));
        return Ok((builder, url));
    }

    if let Some(raw) = auth_params {
        let params: ApiKeyAuthParams =
            serde_json::from_str(raw).map_err(|_| "invalid authParams".to_string())?;
        let location = params.location.trim().to_ascii_lowercase();
        if location == "query" {
            url = with_query_param(url.as_str(), params.name.trim(), secret.trim());
            return Ok((builder, url));
        }
        let value_format = params
            .header_value_format
            .as_deref()
            .unwrap_or("bearer")
            .trim()
            .to_ascii_lowercase();
        let header_value = if value_format == "raw" {
            secret.trim().to_string()
        } else {
            format!("Bearer {}", secret.trim())
        };
        builder = builder.header(params.name.trim(), header_value);
        return Ok((builder, url));
    }

    let auth_value = format!("Bearer {}", secret.trim());
    builder = builder
        .header(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(auth_value.as_str())
                .map_err(|_| "invalid aggregate api key".to_string())?,
        )
        .header("x-api-key", secret.trim())
        .header("api-key", secret.trim());
    Ok((builder, url))
}

/// 函数 `normalize_provider_type`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
///
/// # 返回
/// 返回函数执行结果
fn normalize_provider_type(value: Option<String>) -> Result<String, String> {
    match value {
        Some(raw) => {
            let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
            match normalized.as_str() {
                "codex" | "openai" | "openai_compat" | "gpt" | "gemini" | "gemini_native" => {
                    Ok(AGGREGATE_API_PROVIDER_CODEX.to_string())
                }
                "claude" | "anthropic" | "anthropic_native" | "claude_code" => {
                    Ok(AGGREGATE_API_PROVIDER_CLAUDE.to_string())
                }
                other => Err(format!("unsupported aggregate api provider type: {other}")),
            }
        }
        None => Ok(AGGREGATE_API_PROVIDER_CODEX.to_string()),
    }
}

/// 函数 `normalize_provider_type_value`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
///
/// # 返回
/// 返回函数执行结果
fn normalize_provider_type_value(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "claude" | "anthropic" | "anthropic_native" | "claude_code" => {
            AGGREGATE_API_PROVIDER_CLAUDE.to_string()
        }
        _ => AGGREGATE_API_PROVIDER_CODEX.to_string(),
    }
}

/// 函数 `provider_default_url`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - provider_type: 参数 provider_type
///
/// # 返回
/// 返回函数执行结果
fn provider_default_url(provider_type: &str) -> &'static str {
    match provider_type {
        AGGREGATE_API_PROVIDER_CLAUDE => "https://api.anthropic.com/v1",
        _ => "https://api.openai.com/v1",
    }
}

/// 函数 `normalize_probe_url`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - base_url: 参数 base_url
/// - suffix: 参数 suffix
///
/// # 返回
/// 返回函数执行结果
fn normalize_probe_url(base_url: &str, suffix: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if suffix.trim().is_empty() {
        return base.to_string();
    }
    if base.ends_with("/v1") {
        format!("{base}{suffix}")
    } else {
        format!("{base}/v1{suffix}")
    }
}

/// 函数 `read_first_chunk`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - response: 参数 response
///
/// # 返回
/// 返回函数执行结果
fn read_first_chunk(mut response: reqwest::blocking::Response) -> Result<(), String> {
    let mut buf = [0u8; 16];
    let read = response.read(&mut buf).map_err(|err| err.to_string())?;
    if read > 0 {
        Ok(())
    } else {
        Err("No response data received".to_string())
    }
}

fn normalize_aggregate_api_models_payload(
    payload: &serde_json::Value,
) -> Result<Vec<AggregateApiModel>, String> {
    let items = payload
        .get("data")
        .and_then(|value| value.as_array())
        .or_else(|| payload.get("models").and_then(|value| value.as_array()))
        .or_else(|| payload.as_array())
        .ok_or_else(|| "aggregate api models payload must contain an array".to_string())?;

    let mut models = Vec::new();
    let now = now_ts();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let slug = obj
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "aggregate api model item missing id".to_string())?;
        models.push(AggregateApiModel {
            aggregate_api_id: String::new(),
            model_slug: slug.to_string(),
            display_name: Some(slug.to_string()),
            raw_json: serde_json::to_string(item)
                .map_err(|_| "aggregate api model item is invalid".to_string())?,
            created_at: now,
            updated_at: now,
        });
    }
    Ok(models)
}

fn normalize_selected_aggregate_api_models(
    aggregate_api_id: &str,
    payload: &serde_json::Value,
) -> Result<Vec<AggregateApiModel>, String> {
    let items = payload
        .as_array()
        .ok_or_else(|| "aggregate api selected models must be an array".to_string())?;
    let now = now_ts();
    let mut seen = HashSet::new();
    let mut models = Vec::new();
    for item in items {
        let parsed: AggregateApiModelSelectionInput = serde_json::from_value(item.clone())
            .map_err(|_| "aggregate api selected model item is invalid".to_string())?;
        let slug = parsed.model_slug.trim();
        if slug.is_empty() || !seen.insert(slug.to_string()) {
            continue;
        }
        let display_name = parsed
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| Some(slug.to_string()));
        let raw_json = parsed
            .raw_json
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                serde_json::to_string(&json!({
                    "id": slug,
                    "display_name": display_name.clone().unwrap_or_else(|| slug.to_string()),
                }))
                .unwrap_or_else(|_| format!("{{\"id\":\"{slug}\"}}"))
            });
        models.push(AggregateApiModel {
            aggregate_api_id: aggregate_api_id.to_string(),
            model_slug: slug.to_string(),
            display_name,
            raw_json,
            created_at: now,
            updated_at: now,
        });
    }
    Ok(models)
}

/// 函数 `build_claude_probe_body`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 返回函数执行结果
fn build_claude_probe_body() -> serde_json::Value {
    json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 1,
        "messages": [{
            "role": "user",
            "content": "Who are you?"
        }],
        "stream": true
    })
}

fn build_claude_model_test_body(model: &str) -> serde_json::Value {
    json!({
        "model": model,
        "max_tokens": 1,
        "messages": [{
            "role": "user",
            "content": "hi"
        }],
        "stream": false
    })
}

fn default_codex_probe_model(endpoint_kind: AggregateApiEndpointKind) -> &'static str {
    match endpoint_kind {
        AggregateApiEndpointKind::ChatCompletions => "gpt-4o-mini",
        _ => "gpt-5.1-codex",
    }
}

fn preferred_codex_probe_model(models: &[AggregateApiModel]) -> Option<String> {
    models
        .iter()
        .map(|item| item.model_slug.trim())
        .find(|model| !model.is_empty())
        .map(str::to_string)
}

fn build_codex_model_test_body(
    endpoint_kind: AggregateApiEndpointKind,
    model: &str,
) -> serde_json::Value {
    if endpoint_kind == AggregateApiEndpointKind::ChatCompletions {
        json!({
            "model": model,
            "messages": [{"role":"user","content":"hi"}],
            "stream": false
        })
    } else {
        json!({
            "model": model,
            "input": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "hi"
                }]
            }],
            "stream": false
        })
    }
}

/// 函数 `append_client_version_query`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - url: 参数 url
///
/// # 返回
/// 返回函数执行结果
fn append_client_version_query(url: &str) -> String {
    if url.contains("client_version=") {
        return url.to_string();
    }
    let separator = if url.contains('?') { '&' } else { '?' };
    format!(
        "{url}{separator}client_version={}",
        gateway::current_codex_user_agent_version()
    )
}

/// 函数 `probe_codex_only_for_provider`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - provider_type: 参数 provider_type
///
/// # 返回
/// 返回函数执行结果
fn probe_codex_only_for_provider(provider_type: &str) -> bool {
    provider_type != AGGREGATE_API_PROVIDER_CLAUDE
}

/// 函数 `add_codex_probe_headers`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - builder: 参数 builder
/// - secret: 参数 secret
///
/// # 返回
/// 返回函数执行结果
fn add_codex_probe_headers(
    builder: reqwest::blocking::RequestBuilder,
) -> Result<reqwest::blocking::RequestBuilder, String> {
    Ok(builder
        .header("accept", "application/json")
        .header("user-agent", gateway::current_codex_user_agent())
        .header("originator", gateway::current_wire_originator())
        .header("accept-encoding", "identity"))
}

fn send_codex_models_request(
    client: &reqwest::blocking::Client,
    api: &AggregateApi,
    secret: &str,
    url: &str,
) -> Result<reqwest::blocking::Response, String> {
    let builder = client.get(url);
    let (builder, updated_url) = apply_probe_auth(builder, url.to_string(), api, secret)?;
    let builder = if updated_url != url {
        let rebuilt = client.get(updated_url.as_str());
        let (rebuilt, _) = apply_probe_auth(rebuilt, updated_url, api, secret)?;
        rebuilt
    } else {
        builder
    };
    add_codex_probe_headers(builder)?
        .send()
        .map_err(|err| err.to_string())
}

fn legacy_root_models_retry_url(api: &AggregateApi, current_url: &str) -> Option<String> {
    let models_path = api.models_path.as_deref().map(str::trim)?;
    if models_path != "/models" {
        return None;
    }
    let base_path = reqwest::Url::parse(api.url.as_str())
        .ok()
        .map(|url| normalize_path_part(url.path()))
        .unwrap_or_else(|| "/".to_string());
    if base_path != "/" {
        return None;
    }
    let mut retry_api = api.clone();
    retry_api.models_path = Some(AGGREGATE_API_REQUEST_MODELS_PATH.to_string());
    let retry_url =
        build_aggregate_api_endpoint_url(&retry_api, AggregateApiEndpointKind::Models).ok()?;
    if retry_url == current_url {
        None
    } else {
        Some(retry_url)
    }
}

/// 函数 `probe_codex_models_endpoint`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - client: 参数 client
/// - base_url: 参数 base_url
/// - secret: 参数 secret
///
/// # 返回
/// 返回函数执行结果
fn probe_codex_models_endpoint(
    client: &reqwest::blocking::Client,
    api: &AggregateApi,
    secret: &str,
) -> Result<i64, String> {
    let base_url = build_aggregate_api_endpoint_url(api, AggregateApiEndpointKind::Models)?;
    let url = append_client_version_query(base_url.as_str());
    let mut response = send_codex_models_request(client, api, secret, url.as_str())?;
    if !response.status().is_success() {
        if let Some(retry_base_url) = legacy_root_models_retry_url(api, base_url.as_str()) {
            let retry_url = append_client_version_query(retry_base_url.as_str());
            response = send_codex_models_request(client, api, secret, retry_url.as_str())?;
        }
    }

    let status_code = response.status().as_u16() as i64;
    if !response.status().is_success() {
        return Err(format!("codex models probe http_status={status_code}"));
    }
    read_first_chunk(response)?;
    Ok(status_code)
}

/// 函数 `probe_codex_responses_endpoint`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - client: 参数 client
/// - base_url: 参数 base_url
/// - secret: 参数 secret
///
/// # 返回
/// 返回函数执行结果
fn probe_codex_responses_endpoint(
    client: &reqwest::blocking::Client,
    api: &AggregateApi,
    secret: &str,
    preferred_model: Option<&str>,
) -> Result<i64, String> {
    let endpoint_kind = if api
        .chat_completions_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || api.upstream_format == "chat_completions"
    {
        AggregateApiEndpointKind::ChatCompletions
    } else {
        AggregateApiEndpointKind::Responses
    };
    let url = build_aggregate_api_endpoint_url(api, endpoint_kind)?;
    let builder = client.post(url.as_str());
    let (builder, updated_url) = apply_probe_auth(builder, url.clone(), api, secret)?;
    let builder = if updated_url != url {
        let rebuilt = client.post(updated_url.as_str());
        let (rebuilt, _) = apply_probe_auth(rebuilt, updated_url, api, secret)?;
        rebuilt
    } else {
        builder
    };
    let request_body = build_codex_model_test_body(
        endpoint_kind,
        preferred_model.unwrap_or(default_codex_probe_model(endpoint_kind)),
    );
    let response = add_codex_probe_headers(builder)?
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&request_body)
        .send()
        .map_err(|err| err.to_string())?;

    let status_code = response.status().as_u16() as i64;
    if !response.status().is_success() {
        return Err(format!("codex probe http_status={status_code}"));
    }
    read_first_chunk(response)?;
    Ok(status_code)
}

fn probe_codex_model_endpoint(
    client: &reqwest::blocking::Client,
    api: &AggregateApi,
    secret: &str,
    model: &str,
) -> Result<i64, String> {
    let endpoint_kind = if api
        .chat_completions_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || api.upstream_format == "chat_completions"
    {
        AggregateApiEndpointKind::ChatCompletions
    } else {
        AggregateApiEndpointKind::Responses
    };
    let url = build_aggregate_api_endpoint_url(api, endpoint_kind)?;
    let builder = client.post(url.as_str());
    let (builder, updated_url) = apply_probe_auth(builder, url.clone(), api, secret)?;
    let builder = if updated_url != url {
        let rebuilt = client.post(updated_url.as_str());
        let (rebuilt, _) = apply_probe_auth(rebuilt, updated_url, api, secret)?;
        rebuilt
    } else {
        builder
    };
    let request_body = build_codex_model_test_body(endpoint_kind, model);
    let response = add_codex_probe_headers(builder)?
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&request_body)
        .send()
        .map_err(|err| err.to_string())?;

    let status_code = response.status().as_u16() as i64;
    if !response.status().is_success() {
        return Err(format!("codex model test http_status={status_code}"));
    }
    read_first_chunk(response)?;
    Ok(status_code)
}

/// 函数 `probe_codex_endpoint`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - client: 参数 client
/// - base_url: 参数 base_url
/// - secret: 参数 secret
///
/// # 返回
/// 返回函数执行结果
fn probe_codex_endpoint(
    client: &reqwest::blocking::Client,
    api: &AggregateApi,
    secret: &str,
    preferred_model: Option<&str>,
) -> Result<i64, String> {
    let models_result = probe_codex_models_endpoint(client, api, secret);
    if let Ok(code) = models_result {
        return Ok(code);
    }

    let models_err = models_result
        .err()
        .unwrap_or_else(|| "codex models probe failed".to_string());
    let responses_result = probe_codex_responses_endpoint(client, api, secret, preferred_model);
    if let Ok(code) = responses_result {
        return Ok(code);
    }

    let responses_err = responses_result
        .err()
        .unwrap_or_else(|| "codex responses probe failed".to_string());
    Err(format!("{models_err}; {responses_err}"))
}

fn fetch_aggregate_models_from_upstream(
    client: &reqwest::blocking::Client,
    api: &AggregateApi,
    secret: &str,
) -> Result<Vec<AggregateApiModel>, String> {
    let base_url = build_aggregate_api_endpoint_url(api, AggregateApiEndpointKind::Models)?;
    let url = append_client_version_query(base_url.as_str());
    let mut response = send_codex_models_request(client, api, secret, url.as_str())?;
    if !response.status().is_success() {
        if let Some(retry_base_url) = legacy_root_models_retry_url(api, base_url.as_str()) {
            let retry_url = append_client_version_query(retry_base_url.as_str());
            response = send_codex_models_request(client, api, secret, retry_url.as_str())?;
        }
    }
    let status_code = response.status().as_u16() as i64;
    if !response.status().is_success() {
        return Err(format!(
            "aggregate api models fetch http_status={status_code}"
        ));
    }
    let payload = response
        .json::<serde_json::Value>()
        .map_err(|err| format!("aggregate api models payload parse failed: {err}"))?;
    let mut models = normalize_aggregate_api_models_payload(&payload)?;
    for model in &mut models {
        model.aggregate_api_id = api.id.clone();
    }
    Ok(models)
}

/// 函数 `probe_claude_endpoint`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - client: 参数 client
/// - base_url: 参数 base_url
/// - secret: 参数 secret
///
/// # 返回
/// 返回函数执行结果
fn probe_claude_endpoint(
    client: &reqwest::blocking::Client,
    api: &AggregateApi,
    secret: &str,
) -> Result<i64, String> {
    let probe_path = action_path_or_default(api, "/messages?beta=true");
    let url = normalize_probe_url(api.url.as_str(), probe_path.as_str());
    let builder = client.post(url.as_str());
    let (builder, updated_url) = apply_probe_auth(builder, url.clone(), api, secret)?;
    let builder = if updated_url != url {
        let rebuilt = client.post(updated_url.as_str());
        let (rebuilt, _) = apply_probe_auth(rebuilt, updated_url, api, secret)?;
        rebuilt
    } else {
        builder
    };
    let response = builder
        .header("anthropic-version", "2023-06-01")
        .header(
            "anthropic-beta",
            "claude-code-20250219,interleaved-thinking-2025-05-14",
        )
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .header("accept-encoding", "identity")
        .header("user-agent", "claude-cli/2.1.2 (external, cli)")
        .header("x-app", "cli")
        .json(&build_claude_probe_body())
        .send()
        .map_err(|err| err.to_string())?;

    let status_code = response.status().as_u16() as i64;
    if !response.status().is_success() {
        return Err(format!("claude probe http_status={status_code}"));
    }
    read_first_chunk(response)?;
    Ok(status_code)
}

fn probe_claude_model_endpoint(
    client: &reqwest::blocking::Client,
    api: &AggregateApi,
    secret: &str,
    model: &str,
) -> Result<i64, String> {
    let probe_path = action_path_or_default(api, "/messages?beta=true");
    let url = normalize_probe_url(api.url.as_str(), probe_path.as_str());
    let builder = client.post(url.as_str());
    let (builder, updated_url) = apply_probe_auth(builder, url.clone(), api, secret)?;
    let builder = if updated_url != url {
        let rebuilt = client.post(updated_url.as_str());
        let (rebuilt, _) = apply_probe_auth(rebuilt, updated_url, api, secret)?;
        rebuilt
    } else {
        builder
    };
    let response = builder
        .header("anthropic-version", "2023-06-01")
        .header(
            "anthropic-beta",
            "claude-code-20250219,interleaved-thinking-2025-05-14",
        )
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .header("accept-encoding", "identity")
        .header("user-agent", "claude-cli/2.1.2 (external, cli)")
        .header("x-app", "cli")
        .json(&build_claude_model_test_body(model))
        .send()
        .map_err(|err| err.to_string())?;

    let status_code = response.status().as_u16() as i64;
    if !response.status().is_success() {
        return Err(format!("claude model test http_status={status_code}"));
    }
    read_first_chunk(response)?;
    Ok(status_code)
}

/// 函数 `list_aggregate_apis`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - crate: 参数 crate
///
/// # 返回
/// 返回函数执行结果
pub(crate) fn list_aggregate_apis() -> Result<Vec<AggregateApiSummary>, String> {
    let storage = open_storage().ok_or_else(|| "open storage failed".to_string())?;
    let items = storage
        .list_aggregate_apis()
        .map_err(|err| format!("list aggregate apis failed: {err}"))?;
    Ok(items
        .into_iter()
        .map(|item| AggregateApiSummary {
            id: item.id,
            provider_type: item.provider_type,
            supplier_name: item.supplier_name,
            sort: item.sort,
            url: item.url,
            auth_type: item.auth_type,
            auth_params: item
                .auth_params_json
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok()),
            action: item.action,
            upstream_format: item.upstream_format,
            models_path: item.models_path,
            responses_path: item.responses_path,
            chat_completions_path: item.chat_completions_path,
            proxy_mode: item.proxy_mode,
            proxy_url: item.proxy_url,
            status: item.status,
            created_at: item.created_at,
            updated_at: item.updated_at,
            last_test_at: item.last_test_at,
            last_test_status: item.last_test_status,
            last_test_error: item.last_test_error,
            models_last_synced_at: item.models_last_synced_at,
            models_last_sync_status: item.models_last_sync_status,
            models_last_sync_error: item.models_last_sync_error,
        })
        .collect())
}

/// 函数 `create_aggregate_api`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - crate: 参数 crate
///
/// # 返回
/// 返回函数执行结果
pub(crate) fn create_aggregate_api(
    url: Option<String>,
    key: Option<String>,
    provider_type: Option<String>,
    supplier_name: Option<String>,
    sort: Option<i64>,
    auth_type: Option<String>,
    auth_custom_enabled: Option<bool>,
    auth_params: Option<serde_json::Value>,
    action_custom_enabled: Option<bool>,
    action: Option<String>,
    upstream_format: Option<String>,
    models_path: Option<String>,
    responses_path: Option<String>,
    chat_completions_path: Option<String>,
    proxy_mode: Option<String>,
    proxy_url: Option<String>,
    username: Option<String>,
    password: Option<String>,
) -> Result<AggregateApiCreateResult, String> {
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let normalized_provider_type = normalize_provider_type(provider_type)?;
    let normalized_supplier_name = normalize_supplier_name(supplier_name)?;
    let normalized_sort = normalize_sort(sort);
    let normalized_url = normalize_upstream_base_url(url)?
        .unwrap_or_else(|| provider_default_url(normalized_provider_type.as_str()).to_string());
    let normalized_auth_type = normalize_auth_type(auth_type)?;
    let normalized_auth_params_json = normalize_auth_params_json(
        normalized_auth_type.as_str(),
        auth_custom_enabled,
        auth_params,
    )?;
    let normalized_action =
        normalize_action_override(action_custom_enabled, action)?.unwrap_or(None);
    let normalized_upstream_format = normalize_upstream_format(upstream_format)?;
    let normalized_models_path =
        normalize_models_path(models_path)?.or_else(|| Some("/v1/models".to_string()));
    let normalized_responses_path = normalize_responses_path(responses_path)?;
    let normalized_chat_completions_path = normalize_chat_completions_path(chat_completions_path)?;
    let normalized_proxy_mode = normalize_proxy_mode(proxy_mode)?;
    let normalized_proxy_url = if normalized_proxy_mode == AGGREGATE_API_PROXY_MODE_CUSTOM {
        normalize_proxy_url(proxy_url)?
            .ok_or_else(|| "proxyUrl is required when proxyMode=custom".to_string())
            .map(Some)?
    } else {
        None
    };
    let normalized_secret = if normalized_auth_type == AGGREGATE_API_AUTH_APIKEY {
        normalize_secret(key).ok_or_else(|| "key is required".to_string())?
    } else {
        let username = username
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "username is required".to_string())?;
        let password = password
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "password is required".to_string())?;
        serialize_userpass_secret(username, password)?
    };
    let id = generate_aggregate_api_id();
    let created_at = now_ts();
    let record = AggregateApi {
        id: id.clone(),
        provider_type: normalized_provider_type,
        supplier_name: Some(normalized_supplier_name),
        sort: normalized_sort,
        url: normalized_url,
        auth_type: normalized_auth_type,
        auth_params_json: normalized_auth_params_json
            .map(|value| if value.is_empty() { None } else { Some(value) })
            .unwrap_or(None),
        action: normalized_action,
        upstream_format: normalized_upstream_format,
        models_path: normalized_models_path,
        responses_path: normalized_responses_path,
        chat_completions_path: normalized_chat_completions_path,
        proxy_mode: normalized_proxy_mode,
        proxy_url: normalized_proxy_url,
        status: "active".to_string(),
        created_at,
        updated_at: created_at,
        last_test_at: None,
        last_test_status: None,
        last_test_error: None,
        models_last_synced_at: None,
        models_last_sync_status: None,
        models_last_sync_error: None,
    };
    storage
        .insert_aggregate_api(&record)
        .map_err(|err| err.to_string())?;
    if let Err(err) = storage.upsert_aggregate_api_secret(&id, &normalized_secret) {
        let _ = storage.delete_aggregate_api(&id);
        return Err(format!("persist aggregate api secret failed: {err}"));
    }
    Ok(AggregateApiCreateResult {
        id,
        key: if record.auth_type == AGGREGATE_API_AUTH_APIKEY {
            normalized_secret
        } else {
            String::new()
        },
    })
}

/// 函数 `update_aggregate_api`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - crate: 参数 crate
///
/// # 返回
/// 返回函数执行结果
pub(crate) fn update_aggregate_api(
    api_id: &str,
    url: Option<String>,
    key: Option<String>,
    provider_type: Option<String>,
    supplier_name: Option<String>,
    sort: Option<i64>,
    status: Option<String>,
    auth_type: Option<String>,
    auth_custom_enabled: Option<bool>,
    auth_params: Option<serde_json::Value>,
    action_custom_enabled: Option<bool>,
    action: Option<String>,
    upstream_format: Option<String>,
    models_path: Option<String>,
    responses_path: Option<String>,
    chat_completions_path: Option<String>,
    proxy_mode: Option<String>,
    proxy_url: Option<String>,
    username: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    if api_id.is_empty() {
        return Err("aggregate api id required".to_string());
    }
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let existing = storage
        .find_aggregate_api_by_id(api_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aggregate api not found".to_string())?;
    let existing_auth_type = normalize_auth_type(Some(existing.auth_type.clone()))
        .unwrap_or_else(|_| AGGREGATE_API_AUTH_APIKEY.to_string());
    let normalized_auth_type = match auth_type {
        Some(raw) => Some(normalize_auth_type(Some(raw))?),
        None => None,
    };
    let next_auth_type = normalized_auth_type
        .as_deref()
        .unwrap_or(existing_auth_type.as_str())
        .to_string();
    let auth_type_changed = next_auth_type != existing_auth_type;

    if let Some(next) = normalized_auth_type.as_deref() {
        storage
            .update_aggregate_api_auth_type(api_id, next)
            .map_err(|err| err.to_string())?;
    }
    if let Some(provider_type) = provider_type {
        let normalized_provider_type = normalize_provider_type(Some(provider_type))?;
        storage
            .update_aggregate_api_type(api_id, normalized_provider_type.as_str())
            .map_err(|err| err.to_string())?;
    }
    let normalized_supplier_name = normalize_supplier_name(supplier_name)?;
    storage
        .update_aggregate_api_supplier_name(api_id, Some(normalized_supplier_name.as_str()))
        .map_err(|err| err.to_string())?;
    if sort.is_some() {
        storage
            .update_aggregate_api_sort(api_id, normalize_sort(sort))
            .map_err(|err| err.to_string())?;
    }
    if let Some(status) = status {
        let normalized_status = normalize_status(Some(status))?;
        storage
            .update_aggregate_api_status(api_id, normalized_status.as_str())
            .map_err(|err| err.to_string())?;
    }
    if let Some(url) = url {
        let normalized_url =
            normalize_upstream_base_url(Some(url))?.ok_or_else(|| "url is required".to_string())?;
        storage
            .update_aggregate_api(api_id, normalized_url.as_str())
            .map_err(|err| err.to_string())?;
    }

    if let Some(auth_params_json) =
        normalize_auth_params_json(next_auth_type.as_str(), auth_custom_enabled, auth_params)?
    {
        let normalized = auth_params_json.trim().to_string();
        if normalized.is_empty() {
            storage
                .update_aggregate_api_auth_params_json(api_id, None)
                .map_err(|err| err.to_string())?;
        } else {
            storage
                .update_aggregate_api_auth_params_json(api_id, Some(normalized.as_str()))
                .map_err(|err| err.to_string())?;
        }
    }

    if let Some(action_override) = normalize_action_override(action_custom_enabled, action)? {
        if let Some(action) = action_override {
            let normalized = action.trim().to_string();
            storage
                .update_aggregate_api_action(api_id, Some(normalized.as_str()))
                .map_err(|err| err.to_string())?;
        } else {
            storage
                .update_aggregate_api_action(api_id, None)
                .map_err(|err| err.to_string())?;
        }
    }

    if let Some(raw) = upstream_format {
        let normalized = normalize_upstream_format(Some(raw))?;
        storage
            .update_aggregate_api_upstream_format(api_id, normalized.as_str())
            .map_err(|err| err.to_string())?;
    }
    if let Some(normalized) = normalize_models_path(models_path)? {
        storage
            .update_aggregate_api_models_path(api_id, Some(normalized.as_str()))
            .map_err(|err| err.to_string())?;
    }
    if responses_path.is_some() {
        let normalized = normalize_responses_path(responses_path)?;
        storage
            .update_aggregate_api_responses_path(api_id, normalized.as_deref())
            .map_err(|err| err.to_string())?;
    }
    if chat_completions_path.is_some() {
        let normalized = normalize_chat_completions_path(chat_completions_path)?;
        storage
            .update_aggregate_api_chat_completions_path(api_id, normalized.as_deref())
            .map_err(|err| err.to_string())?;
    }
    if let Some(raw_proxy_mode) = proxy_mode {
        let normalized = normalize_proxy_mode(Some(raw_proxy_mode))?;
        storage
            .update_aggregate_api_proxy_mode(api_id, normalized.as_str())
            .map_err(|err| err.to_string())?;
        let normalized_proxy_url = if normalized == AGGREGATE_API_PROXY_MODE_CUSTOM {
            normalize_proxy_url(proxy_url)?
                .ok_or_else(|| "proxyUrl is required when proxyMode=custom".to_string())
                .map(Some)?
        } else {
            None
        };
        storage
            .update_aggregate_api_proxy_url(api_id, normalized_proxy_url.as_deref())
            .map_err(|err| err.to_string())?;
    } else if proxy_url.is_some() {
        let active_proxy_mode = normalize_proxy_mode(Some(existing.proxy_mode.clone()))?;
        if active_proxy_mode != AGGREGATE_API_PROXY_MODE_CUSTOM {
            return Err("proxyUrl requires proxyMode=custom".to_string());
        }
        let normalized = normalize_proxy_url(proxy_url)?;
        storage
            .update_aggregate_api_proxy_url(api_id, normalized.as_deref())
            .map_err(|err| err.to_string())?;
    }

    if next_auth_type == AGGREGATE_API_AUTH_APIKEY {
        let normalized_secret = normalize_secret(key);
        if auth_type_changed && normalized_secret.is_none() {
            return Err("key is required when switching authType to apikey".to_string());
        }
        if let Some(secret) = normalized_secret {
            storage
                .upsert_aggregate_api_secret(api_id, &secret)
                .map_err(|err| err.to_string())?;
        }
    } else {
        let username = username.as_deref().map(str::trim).unwrap_or("");
        let password = password.as_deref().map(str::trim).unwrap_or("");
        let has_user = !username.is_empty();
        let has_pass = !password.is_empty();
        if (has_user && !has_pass) || (!has_user && has_pass) {
            return Err("username and password must be provided together".to_string());
        }
        if auth_type_changed && (!has_user || !has_pass) {
            return Err(
                "username and password are required when switching authType to userpass"
                    .to_string(),
            );
        }
        if has_user && has_pass {
            let secret = serialize_userpass_secret(username, password)?;
            storage
                .upsert_aggregate_api_secret(api_id, &secret)
                .map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn list_aggregate_api_models(
    api_id: &str,
) -> Result<Vec<AggregateApiModelSummary>, String> {
    if api_id.is_empty() {
        return Err("aggregate api id required".to_string());
    }
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let items = storage
        .list_aggregate_api_models(api_id)
        .map_err(|err| err.to_string())?;
    Ok(items
        .into_iter()
        .map(|item| AggregateApiModelSummary {
            aggregate_api_id: item.aggregate_api_id,
            model_slug: item.model_slug,
            display_name: item.display_name,
            updated_at: item.updated_at,
        })
        .collect())
}

pub(crate) fn fetch_aggregate_api_models(
    api_id: &str,
) -> Result<AggregateApiFetchModelsResult, String> {
    if api_id.is_empty() {
        return Err("aggregate api id required".to_string());
    }
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let api = storage
        .find_aggregate_api_by_id(api_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aggregate api not found".to_string())?;
    let secret = storage
        .find_aggregate_api_secret_by_id(api_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aggregate api secret not found".to_string())?;

    let client = build_aggregate_api_client(&api)?;
    match fetch_aggregate_models_from_upstream(&client, &api, secret.as_str()) {
        Ok(models) => {
            let fetched_at = now_ts();
            Ok(AggregateApiFetchModelsResult {
                id: api_id.to_string(),
                count: models.len() as i64,
                fetched_at,
                items: models
                    .into_iter()
                    .map(|item| AggregateApiFetchedModelSummary {
                        aggregate_api_id: item.aggregate_api_id,
                        model_slug: item.model_slug,
                        display_name: item.display_name,
                        raw_json: Some(item.raw_json),
                        updated_at: item.updated_at,
                    })
                    .collect(),
            })
        }
        Err(err) => {
            let _ = storage.update_aggregate_api_models_sync_result(
                api_id,
                Some(now_ts()),
                Some("failed"),
                Some(err.as_str()),
            );
            Err(err)
        }
    }
}

pub(crate) fn save_aggregate_api_models(
    api_id: &str,
    items: Option<serde_json::Value>,
) -> Result<AggregateApiSaveModelsResult, String> {
    if api_id.is_empty() {
        return Err("aggregate api id required".to_string());
    }
    let payload = items.ok_or_else(|| "selected aggregate api models required".to_string())?;
    let models = normalize_selected_aggregate_api_models(api_id, &payload)?;
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    storage
        .find_aggregate_api_by_id(api_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aggregate api not found".to_string())?;
    let synced_at = now_ts();
    storage
        .replace_aggregate_api_models(api_id, &models)
        .map_err(|err| err.to_string())?;
    storage
        .update_aggregate_api_models_sync_result(api_id, Some(synced_at), Some("success"), None)
        .map_err(|err| err.to_string())?;
    Ok(AggregateApiSaveModelsResult {
        id: api_id.to_string(),
        count: models.len() as i64,
        synced_at,
    })
}

/// 函数 `delete_aggregate_api`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - crate: 参数 crate
///
/// # 返回
/// 返回函数执行结果
pub(crate) fn delete_aggregate_api(api_id: &str) -> Result<(), String> {
    if api_id.is_empty() {
        return Err("aggregate api id required".to_string());
    }
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    storage
        .delete_aggregate_api(api_id)
        .map_err(|err| err.to_string())
}

/// 函数 `read_aggregate_api_secret`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - crate: 参数 crate
///
/// # 返回
/// 返回函数执行结果
pub(crate) fn read_aggregate_api_secret(api_id: &str) -> Result<AggregateApiSecretResult, String> {
    if api_id.is_empty() {
        return Err("aggregate api id required".to_string());
    }
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let api = storage
        .find_aggregate_api_by_id(api_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aggregate api not found".to_string())?;
    let key = storage
        .find_aggregate_api_secret_by_id(api_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aggregate api secret not found".to_string())?;
    let auth_type = normalize_auth_type(Some(api.auth_type))?;
    if auth_type == AGGREGATE_API_AUTH_USERPASS {
        let parsed: UserPassSecret = serde_json::from_str(key.as_str())
            .map_err(|_| "invalid aggregate api secret".to_string())?;
        return Ok(AggregateApiSecretResult {
            id: api_id.to_string(),
            key: String::new(),
            auth_type,
            username: Some(parsed.username),
            password: Some(parsed.password),
        });
    }
    Ok(AggregateApiSecretResult {
        id: api_id.to_string(),
        key,
        auth_type,
        username: None,
        password: None,
    })
}

/// 函数 `test_aggregate_api_connection`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - crate: 参数 crate
///
/// # 返回
/// 返回函数执行结果
pub(crate) fn test_aggregate_api_connection(
    api_id: &str,
) -> Result<AggregateApiTestResult, String> {
    if api_id.is_empty() {
        return Err("aggregate api id required".to_string());
    }
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let api = storage
        .find_aggregate_api_by_id(api_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aggregate api not found".to_string())?;
    let secret = storage
        .find_aggregate_api_secret_by_id(api_id)
        .map_err(|err| err.to_string())?;
    let Some(secret) = secret else {
        return Err("aggregate api secret not found".to_string());
    };
    let client = build_aggregate_api_client(&api)?;
    let started_at = Instant::now();
    let provider_type = normalize_provider_type_value(api.provider_type.as_str());
    let preferred_model = storage
        .list_aggregate_api_models(api_id)
        .ok()
        .and_then(|items| preferred_codex_probe_model(&items));
    let result = if probe_codex_only_for_provider(provider_type.as_str()) {
        probe_codex_endpoint(&client, &api, &secret, preferred_model.as_deref())
    } else {
        probe_claude_endpoint(&client, &api, &secret)
    };
    let (ok, status_code, last_error) = match result {
        Ok(code) => (true, Some(code), None),
        Err(err) => (false, None, Some(err)),
    };
    let message = last_error.map(|err| format!("provider={provider_type}; {err}"));

    let _ = storage.update_aggregate_api_test_result(api_id, ok, status_code, message.as_deref());
    Ok(AggregateApiTestResult {
        id: api_id.to_string(),
        ok,
        status_code,
        message,
        tested_at: now_ts(),
        latency_ms: started_at.elapsed().as_millis() as i64,
    })
}

pub(crate) fn test_aggregate_api_model(
    api_id: &str,
    model: &str,
) -> Result<AggregateApiTestResult, String> {
    if api_id.is_empty() {
        return Err("aggregate api id required".to_string());
    }
    let model = model.trim();
    if model.is_empty() {
        return Err("aggregate api model required".to_string());
    }
    let storage = open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let api = storage
        .find_aggregate_api_by_id(api_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aggregate api not found".to_string())?;
    let secret = storage
        .find_aggregate_api_secret_by_id(api_id)
        .map_err(|err| err.to_string())?;
    let Some(secret) = secret else {
        return Err("aggregate api secret not found".to_string());
    };
    let client = build_aggregate_api_client(&api)?;
    let started_at = Instant::now();
    let provider_type = normalize_provider_type_value(api.provider_type.as_str());
    let result = if probe_codex_only_for_provider(provider_type.as_str()) {
        probe_codex_model_endpoint(&client, &api, &secret, model)
    } else {
        probe_claude_model_endpoint(&client, &api, &secret, model)
    };
    let (ok, status_code, message) = match result {
        Ok(code) => (true, Some(code), Some("模型连通正常".to_string())),
        Err(err) => (
            false,
            None,
            Some(format!("provider={provider_type}; model={model}; {err}")),
        ),
    };
    Ok(AggregateApiTestResult {
        id: api_id.to_string(),
        ok,
        status_code,
        message,
        tested_at: now_ts(),
        latency_ms: started_at.elapsed().as_millis() as i64,
    })
}
