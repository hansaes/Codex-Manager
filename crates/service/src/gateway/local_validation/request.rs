use crate::apikey_profile::{
    is_gemini_generate_content_request_path, resolve_gateway_protocol_type,
    PROTOCOL_ANTHROPIC_NATIVE, PROTOCOL_GEMINI_NATIVE, ROTATION_AGGREGATE_API,
};
use crate::gateway::request_helpers::ParsedRequestMetadata;
use bytes::Bytes;
use codexmanager_core::storage::ApiKey;
use reqwest::Method;
use std::collections::HashSet;
use tiny_http::Request;

use super::{GatewayRouteVariant, LocalValidationError, LocalValidationResult};

/// 函数 `resolve_effective_request_overrides`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - api_key: 参数 api_key
///
/// # 返回
/// 返回函数执行结果
fn resolve_effective_request_overrides(
    api_key: &ApiKey,
) -> (Option<String>, Option<String>, Option<String>) {
    let normalized_model = api_key
        .model_slug
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(super::super::resolve_builtin_forwarded_model)
        .or_else(|| {
            api_key
                .model_slug
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    let normalized_reasoning = api_key
        .reasoning_effort
        .as_deref()
        .and_then(crate::reasoning_effort::normalize_reasoning_effort)
        .map(str::to_string);
    let normalized_service_tier = api_key
        .service_tier
        .as_deref()
        .and_then(crate::apikey::service_tier::normalize_service_tier)
        .map(str::to_string);

    (
        normalized_model,
        normalized_reasoning,
        normalized_service_tier,
    )
}

/// 函数 `ensure_anthropic_model_is_listed`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - storage: 参数 storage
/// - protocol_type: 参数 protocol_type
/// - model: 参数 model
///
/// # 返回
/// 返回函数执行结果
fn ensure_anthropic_model_is_listed(
    storage: &codexmanager_core::storage::Storage,
    protocol_type: &str,
    model: Option<&str>,
) -> Result<(), LocalValidationError> {
    if protocol_type != PROTOCOL_ANTHROPIC_NATIVE {
        return Ok(());
    }

    let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(LocalValidationError::new(
            400,
            crate::gateway::bilingual_error("Claude 模型必填", "claude model is required"),
        ));
    };

    let models = crate::apikey_models::read_model_options_from_storage(storage).map_err(|err| {
        LocalValidationError::new(
            500,
            crate::gateway::bilingual_error(
                "读取模型缓存失败",
                format!("model options cache read failed: {err}"),
            ),
        )
    })?;
    if models.is_empty() {
        return Err(LocalValidationError::new(
            400,
            crate::gateway::bilingual_error(
                "Claude 模型不在模型列表中",
                format!("claude model not found in model list: {model}"),
            ),
        ));
    }
    let found = models
        .models
        .iter()
        .any(|item| item.slug.trim().eq_ignore_ascii_case(model));
    if found {
        Ok(())
    } else {
        Err(LocalValidationError::new(
            400,
            crate::gateway::bilingual_error(
                "Claude 模型不在模型列表中",
                format!("claude model not found in model list: {model}"),
            ),
        ))
    }
}

fn aggregate_allowed_model_slugs(
    storage: &codexmanager_core::storage::Storage,
    api_key: &ApiKey,
    requested_model: Option<&str>,
) -> Result<Option<HashSet<String>>, LocalValidationError> {
    let Some(resolved_catalog) = super::super::aggregate_catalog::resolve_aggregate_model_catalog(
        storage,
        api_key,
        requested_model,
    )
    .map_err(|err| {
        LocalValidationError::new(
            500,
            crate::gateway::bilingual_error(
                "读取聚合模型失败",
                format!("aggregate model catalog read failed: {err}"),
            ),
        )
    })?
    else {
        return Ok(None);
    };
    Ok(Some(
        resolved_catalog
            .models
            .into_iter()
            .map(|item| item.model_slug.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect(),
    ))
}

fn ensure_aggregate_model_is_allowed(
    storage: &codexmanager_core::storage::Storage,
    api_key: &ApiKey,
    model: Option<&str>,
) -> Result<(), LocalValidationError> {
    let Some(allowed_models) = aggregate_allowed_model_slugs(storage, api_key, model)? else {
        return Ok(());
    };
    let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    if allowed_models.contains(&model.to_ascii_lowercase()) {
        Ok(())
    } else {
        Err(LocalValidationError::new(
            400,
            crate::gateway::bilingual_error(
                "模型不在聚合 API 已选择列表中",
                format!("aggregate model not found in selected catalog: {model}"),
            ),
        ))
    }
}

/// 函数 `allow_openai_responses_path_rewrite`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - protocol_type: 参数 protocol_type
/// - normalized_path: 参数 normalized_path
///
/// # 返回
/// 返回函数执行结果
fn allow_compat_responses_path_rewrite(protocol_type: &str, normalized_path: &str) -> bool {
    (protocol_type == crate::apikey_profile::PROTOCOL_OPENAI_COMPAT
        && (normalized_path.starts_with("/v1/chat/completions")
            || normalized_path.starts_with("/v1/completions")))
        || (protocol_type == PROTOCOL_GEMINI_NATIVE
            && is_gemini_generate_content_request_path(normalized_path))
}

/// 函数 `should_derive_compat_conversation_anchor`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - protocol_type: 参数 protocol_type
/// - normalized_path: 参数 normalized_path
///
/// # 返回
/// 返回函数执行结果
fn should_derive_compat_conversation_anchor(protocol_type: &str, normalized_path: &str) -> bool {
    (protocol_type == PROTOCOL_ANTHROPIC_NATIVE && normalized_path.starts_with("/v1/messages"))
        || allow_compat_responses_path_rewrite(protocol_type, normalized_path)
}

/// 函数 `is_native_codex_client_request`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-16
///
/// # 参数
/// - incoming_headers: 参数 incoming_headers
///
/// # 返回
/// 返回函数执行结果
fn is_native_codex_client_request(incoming_headers: &super::super::IncomingHeaderSnapshot) -> bool {
    let user_agent = incoming_headers
        .user_agent()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let originator = incoming_headers
        .originator()
        .unwrap_or_default()
        .to_ascii_lowercase();

    let has_codex_header_signals = incoming_headers.client_request_id().is_some()
        || incoming_headers.subagent().is_some()
        || incoming_headers.beta_features().is_some()
        || incoming_headers.window_id().is_some()
        || incoming_headers.turn_metadata().is_some()
        || incoming_headers.turn_state().is_some()
        || incoming_headers.parent_thread_id().is_some();

    user_agent.contains("codex_cli_rs")
        || originator.contains("codex_cli_rs")
        || user_agent.contains("codex_exec")
        || originator.contains("codex_exec")
        || has_codex_header_signals
}

/// 函数 `should_force_codex_compat_rewrite`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-16
///
/// # 参数
/// - normalized_path: 参数 normalized_path
/// - incoming_headers: 参数 incoming_headers
///
/// # 返回
/// 返回函数执行结果
fn should_force_codex_compat_rewrite(normalized_path: &str, native_codex_client: bool) -> bool {
    normalized_path.starts_with("/v1/responses") && !native_codex_client
}

fn should_normalize_compat_service_tier_for_codex_backend(
    protocol_type: &str,
    normalized_path: &str,
    adapted_path: &str,
) -> bool {
    adapted_path.starts_with("/v1/responses")
        && ((protocol_type == PROTOCOL_ANTHROPIC_NATIVE
            && normalized_path.starts_with("/v1/messages"))
            || allow_compat_responses_path_rewrite(protocol_type, normalized_path))
}

fn normalize_compat_service_tier_for_codex_backend(body: Vec<u8>) -> Vec<u8> {
    let Ok(mut payload) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return body;
    };
    let Some(obj) = payload.as_object_mut() else {
        return body;
    };
    let Some(service_tier) = obj.get_mut("service_tier") else {
        return body;
    };
    let Some(raw_value) = service_tier.as_str() else {
        return body;
    };

    if raw_value.eq_ignore_ascii_case("fast") || raw_value.eq_ignore_ascii_case("priority") {
        *service_tier = serde_json::Value::String("priority".to_string());
    } else {
        obj.remove("service_tier");
    }

    serde_json::to_vec(&payload).unwrap_or(body)
}

fn resolve_preferred_client_prompt_cache_key(
    protocol_type: &str,
    incoming_headers: &super::super::IncomingHeaderSnapshot,
    initial_request_meta: &ParsedRequestMetadata,
    client_request_meta: &ParsedRequestMetadata,
) -> Option<String> {
    if protocol_type == PROTOCOL_ANTHROPIC_NATIVE {
        return None;
    }

    let preferred = initial_request_meta.prompt_cache_key.clone().or_else(|| {
        if client_request_meta.has_prompt_cache_key {
            client_request_meta.prompt_cache_key.clone()
        } else {
            None
        }
    });
    let Some(preferred) = preferred else {
        return None;
    };

    if incoming_headers.conversation_id().is_some() || incoming_headers.turn_state().is_some() {
        // 中文注释：原生 Codex 已经提供稳定线程锚点时，prompt_cache_key 不能反客为主；
        // 否则会和 conversation_id / turn-state 冲突，导致 resume 线程异常。
        return None;
    }

    Some(preferred)
}

/// 函数 `resolve_local_conversation_id`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - protocol_type: 参数 protocol_type
/// - normalized_path: 参数 normalized_path
/// - incoming_headers: 参数 incoming_headers
/// - client_has_prompt_cache_key: 参数 client_has_prompt_cache_key
///
/// # 返回
/// 返回函数执行结果
fn resolve_local_conversation_id(
    protocol_type: &str,
    normalized_path: &str,
    incoming_headers: &super::super::IncomingHeaderSnapshot,
    client_has_prompt_cache_key: bool,
) -> Option<String> {
    super::super::resolve_local_conversation_id_with_sticky_fallback(
        incoming_headers,
        !client_has_prompt_cache_key
            && should_derive_compat_conversation_anchor(protocol_type, normalized_path),
    )
}

/// 函数 `apply_passthrough_request_overrides`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - path: 参数 path
/// - body: 参数 body
/// - api_key: 参数 api_key
///
/// # 返回
/// 返回函数执行结果
fn apply_passthrough_request_overrides(
    path: &str,
    body: Vec<u8>,
    api_key: &ApiKey,
    explicit_service_tier_for_log: Option<String>,
) -> (
    Vec<u8>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
    Option<String>,
) {
    let (effective_model, effective_reasoning, effective_service_tier) =
        resolve_effective_request_overrides(api_key);
    let rewritten_body =
        super::super::apply_request_overrides_with_service_tier_and_prompt_cache_key_scope(
            path,
            body,
            effective_model.as_deref(),
            effective_reasoning.as_deref(),
            effective_service_tier.as_deref(),
            api_key.upstream_base_url.as_deref(),
            None,
            false,
        );
    let request_meta = super::super::parse_request_metadata(&rewritten_body);
    (
        rewritten_body,
        request_meta.model.or(api_key.model_slug.clone()),
        request_meta
            .reasoning_effort
            .or(api_key.reasoning_effort.clone()),
        explicit_service_tier_for_log,
        request_meta.service_tier,
        request_meta.has_prompt_cache_key,
        request_meta.request_shape,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_aggregate_route_variant(
    storage: &codexmanager_core::storage::Storage,
    normalized_path: &str,
    body: Vec<u8>,
    api_key: &ApiKey,
    protocol_type: &str,
    incoming_headers: &super::super::IncomingHeaderSnapshot,
    explicit_service_tier_for_log: Option<String>,
    local_conversation_id: Option<&str>,
    is_stream: bool,
) -> Result<GatewayRouteVariant, LocalValidationError> {
    let (
        rewritten_body,
        model_for_log,
        reasoning_for_log,
        service_tier_for_log,
        effective_service_tier_for_log,
        has_prompt_cache_key,
        request_shape,
    ) = apply_passthrough_request_overrides(
        normalized_path,
        body,
        api_key,
        explicit_service_tier_for_log,
    );
    let aggregate_api_id = super::super::aggregate_catalog::resolve_aggregate_model_catalog(
        storage,
        api_key,
        model_for_log.as_deref(),
    )
    .map_err(|err| {
        LocalValidationError::new(
            500,
            crate::gateway::bilingual_error(
                "读取聚合模型失败",
                format!("aggregate model catalog read failed: {err}"),
            ),
        )
    })?
    .and_then(|catalog| catalog.aggregate_api_id)
    .or_else(|| api_key.aggregate_api_id.clone());
    ensure_aggregate_model_is_allowed(storage, api_key, model_for_log.as_deref())?;
    super::super::validate_text_input_limit_for_path(normalized_path, &rewritten_body)
        .map_err(|err| LocalValidationError::new(400, err.message()))?;
    Ok(GatewayRouteVariant {
        incoming_headers: incoming_headers.with_conversation_id_override(local_conversation_id),
        path: normalized_path.to_string(),
        body: Bytes::from(rewritten_body),
        is_stream,
        has_prompt_cache_key,
        request_shape,
        protocol_type: protocol_type.to_string(),
        aggregate_api_id,
        response_adapter: super::super::ResponseAdapter::Passthrough,
        gemini_stream_output_mode: None,
        tool_name_restore_map: super::super::ToolNameRestoreMap::default(),
        conversation_binding: None,
        model_for_log,
        reasoning_for_log,
        service_tier_for_log,
        effective_service_tier_for_log,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_account_route_variant(
    storage: &codexmanager_core::storage::Storage,
    normalized_path: &str,
    body: Vec<u8>,
    api_key: &ApiKey,
    effective_protocol_type: &str,
    incoming_headers: &super::super::IncomingHeaderSnapshot,
    initial_request_meta: &ParsedRequestMetadata,
    native_codex_client: bool,
    local_conversation_id: Option<String>,
) -> Result<GatewayRouteVariant, LocalValidationError> {
    let original_body = body.clone();
    let adapted =
        super::super::adapt_request_for_protocol(effective_protocol_type, normalized_path, body)
            .map_err(|err| {
                LocalValidationError::new(
                    400,
                    crate::gateway::bilingual_error("请求协议适配失败", err),
                )
            })?;
    let mut path = adapted.path;
    let mut response_adapter = adapted.response_adapter;
    let mut gemini_stream_output_mode = adapted.gemini_stream_output_mode;
    let mut tool_name_restore_map = adapted.tool_name_restore_map;
    let mut body = adapted.body;
    if effective_protocol_type != PROTOCOL_ANTHROPIC_NATIVE
        && !normalized_path.starts_with("/v1/responses")
        && path.starts_with("/v1/responses")
        && !allow_compat_responses_path_rewrite(effective_protocol_type, normalized_path)
    {
        log::warn!(
            "event=gateway_protocol_adapt_guard protocol_type={} from_path={} to_path={} action=force_passthrough",
            effective_protocol_type,
            normalized_path,
            path
        );
        path = normalized_path.to_string();
        body = original_body;
        response_adapter = super::super::ResponseAdapter::Passthrough;
        gemini_stream_output_mode = None;
        tool_name_restore_map.clear();
    }

    let client_request_meta = super::super::parse_request_metadata(&body);
    let (effective_model, effective_reasoning, effective_service_tier) =
        resolve_effective_request_overrides(api_key);
    let preferred_prompt_cache_key = resolve_preferred_client_prompt_cache_key(
        effective_protocol_type,
        incoming_headers,
        initial_request_meta,
        &client_request_meta,
    );
    let allow_codex_compat_rewrite =
        allow_compat_responses_path_rewrite(effective_protocol_type, normalized_path)
            || should_force_codex_compat_rewrite(normalized_path, native_codex_client);
    let conversation_binding = super::super::conversation_binding::load_conversation_binding(
        storage,
        api_key.key_hash.as_str(),
        local_conversation_id.as_deref(),
    )
    .map_err(|err| LocalValidationError::new(500, err))?;
    let effective_thread_anchor = super::super::resolve_fallback_thread_anchor(
        incoming_headers,
        local_conversation_id.as_deref(),
        conversation_binding.as_ref(),
    );
    let incoming_headers =
        incoming_headers.with_conversation_id_override(local_conversation_id.as_deref());
    let should_normalize_compat_service_tier = should_normalize_compat_service_tier_for_codex_backend(
        effective_protocol_type,
        normalized_path,
        path.as_str(),
    );
    body = if preferred_prompt_cache_key.is_some() {
        super::super::apply_request_overrides_with_service_tier_and_prompt_cache_key_scope(
            &path,
            body,
            effective_model.as_deref(),
            effective_reasoning.as_deref(),
            effective_service_tier.as_deref(),
            api_key.upstream_base_url.as_deref(),
            preferred_prompt_cache_key.as_deref(),
            allow_codex_compat_rewrite,
        )
    } else if effective_thread_anchor.is_some() {
        super::super::apply_request_overrides_with_service_tier_and_forced_prompt_cache_key_scope(
            &path,
            body,
            effective_model.as_deref(),
            effective_reasoning.as_deref(),
            effective_service_tier.as_deref(),
            api_key.upstream_base_url.as_deref(),
            effective_thread_anchor.as_deref(),
            allow_codex_compat_rewrite,
        )
    } else {
        super::super::apply_request_overrides_with_service_tier_and_prompt_cache_key_scope(
            &path,
            body,
            effective_model.as_deref(),
            effective_reasoning.as_deref(),
            effective_service_tier.as_deref(),
            api_key.upstream_base_url.as_deref(),
            None,
            allow_codex_compat_rewrite,
        )
    };
    if should_normalize_compat_service_tier {
        body = normalize_compat_service_tier_for_codex_backend(body);
    }
    body = super::super::clear_prompt_cache_key_when_native_anchor(&path, body, &incoming_headers);
    super::super::validate_text_input_limit_for_path(&path, &body)
        .map_err(|err| LocalValidationError::new(400, err.message()))?;

    let request_meta = super::super::parse_request_metadata(&body);
    let model_for_log = request_meta.model.or(api_key.model_slug.clone());
    let reasoning_for_log = request_meta
        .reasoning_effort
        .or(api_key.reasoning_effort.clone());
    let service_tier_for_log = client_request_meta.service_tier;
    let effective_service_tier_for_log = request_meta.service_tier;
    let is_stream = client_request_meta.is_stream;
    let has_prompt_cache_key = request_meta.has_prompt_cache_key;
    let request_shape = client_request_meta.request_shape;

    ensure_anthropic_model_is_listed(storage, effective_protocol_type, model_for_log.as_deref())?;

    Ok(GatewayRouteVariant {
        incoming_headers,
        path,
        body: Bytes::from(body),
        is_stream,
        has_prompt_cache_key,
        request_shape,
        protocol_type: effective_protocol_type.to_string(),
        aggregate_api_id: api_key.aggregate_api_id.clone(),
        response_adapter,
        gemini_stream_output_mode,
        tool_name_restore_map,
        conversation_binding,
        model_for_log,
        reasoning_for_log,
        service_tier_for_log,
        effective_service_tier_for_log,
    })
}

/// 函数 `build_local_validation_result`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) fn build_local_validation_result(
    request: &Request,
    trace_id: String,
    incoming_headers: super::super::IncomingHeaderSnapshot,
    storage: crate::storage_helpers::StorageHandle,
    body: Vec<u8>,
    api_key: ApiKey,
) -> Result<LocalValidationResult, LocalValidationError> {
    // 按当前策略取消每次请求都更新 api_keys.last_used_at，减少并发写入冲突。
    let normalized_path = super::super::normalize_models_path(request.url());
    let effective_protocol_type =
        resolve_gateway_protocol_type(api_key.protocol_type.as_str(), normalized_path.as_str());
    let request_method = request.method().as_str().to_string();
    let method = Method::from_bytes(request_method.as_bytes()).map_err(|_| {
        LocalValidationError::new(
            405,
            crate::gateway::bilingual_error("不支持的请求方法", "unsupported method"),
        )
    })?;
    let initial_service_tier_diagnostic = super::super::inspect_service_tier_for_log(&body);
    super::super::log_client_service_tier(
        trace_id.as_str(),
        "http",
        normalized_path.as_str(),
        initial_service_tier_diagnostic.has_field,
        initial_service_tier_diagnostic.raw_value.as_deref(),
        initial_service_tier_diagnostic.normalized_value.as_deref(),
    );
    let initial_request_meta = super::super::parse_request_metadata(&body);
    let native_codex_client = is_native_codex_client_request(&incoming_headers);
    log::debug!(
        "event=gateway_client_profile trace_id={} path={} originator={} user_agent={} session_affinity={} native_codex={}",
        trace_id.as_str(),
        normalized_path.as_str(),
        incoming_headers.originator().unwrap_or("-"),
        incoming_headers.user_agent().unwrap_or("-"),
        incoming_headers.session_affinity().unwrap_or("-"),
        if native_codex_client {
            "true"
        } else {
            "false"
        }
    );
    let initial_local_conversation_id = resolve_local_conversation_id(
        effective_protocol_type,
        normalized_path.as_str(),
        &incoming_headers,
        initial_request_meta.has_prompt_cache_key,
    );
    let raw_request_body = body.clone();
    let global_channel_priority_enabled = crate::gateway::global_channel_priority_enabled();
    let default_is_aggregate = api_key.rotation_strategy == ROTATION_AGGREGATE_API;

    if default_is_aggregate {
        let aggregate_variant = build_aggregate_route_variant(
            &storage,
            normalized_path.as_str(),
            body,
            &api_key,
            effective_protocol_type,
            &incoming_headers,
            initial_request_meta.service_tier.clone(),
            initial_local_conversation_id.as_deref(),
            initial_request_meta.is_stream,
        )?;
        let account_route_variant = if global_channel_priority_enabled {
            build_account_route_variant(
                &storage,
                normalized_path.as_str(),
                raw_request_body.clone(),
                &api_key,
                effective_protocol_type,
                &incoming_headers,
                &initial_request_meta,
                native_codex_client,
                initial_local_conversation_id.clone(),
            )
            .ok()
        } else {
            None
        };

        return Ok(LocalValidationResult {
            trace_id,
            incoming_headers: aggregate_variant.incoming_headers.clone(),
            storage,
            original_path: normalized_path,
            path: aggregate_variant.path.clone(),
            body: aggregate_variant.body.clone(),
            is_stream: aggregate_variant.is_stream,
            has_prompt_cache_key: aggregate_variant.has_prompt_cache_key,
            request_shape: aggregate_variant.request_shape.clone(),
            protocol_type: aggregate_variant.protocol_type.clone(),
            rotation_strategy: api_key.rotation_strategy,
            aggregate_api_id: aggregate_variant.aggregate_api_id.clone(),
            account_plan_filter: api_key.account_plan_filter,
            response_adapter: aggregate_variant.response_adapter,
            gemini_stream_output_mode: aggregate_variant.gemini_stream_output_mode,
            tool_name_restore_map: aggregate_variant.tool_name_restore_map.clone(),
            request_method,
            key_id: api_key.id,
            platform_key_hash: api_key.key_hash,
            local_conversation_id: initial_local_conversation_id,
            conversation_binding: aggregate_variant.conversation_binding.clone(),
            model_for_log: aggregate_variant.model_for_log.clone(),
            reasoning_for_log: aggregate_variant.reasoning_for_log.clone(),
            service_tier_for_log: aggregate_variant.service_tier_for_log.clone(),
            effective_service_tier_for_log: aggregate_variant
                .effective_service_tier_for_log
                .clone(),
            account_route_variant,
            aggregate_route_variant: if global_channel_priority_enabled {
                Some(aggregate_variant.clone())
            } else {
                None
            },
            method,
        });
    }

    let account_variant = build_account_route_variant(
        &storage,
        normalized_path.as_str(),
        raw_request_body.clone(),
        &api_key,
        effective_protocol_type,
        &incoming_headers,
        &initial_request_meta,
        native_codex_client,
        initial_local_conversation_id.clone(),
    )?;
    let aggregate_route_variant = if global_channel_priority_enabled {
        build_aggregate_route_variant(
            &storage,
            normalized_path.as_str(),
            raw_request_body,
            &api_key,
            effective_protocol_type,
            &incoming_headers,
            initial_request_meta.service_tier.clone(),
            initial_local_conversation_id.as_deref(),
            initial_request_meta.is_stream,
        )
        .ok()
    } else {
        None
    };

    Ok(LocalValidationResult {
        trace_id,
        incoming_headers: account_variant.incoming_headers.clone(),
        storage,
        original_path: normalized_path,
        path: account_variant.path.clone(),
        body: account_variant.body.clone(),
        is_stream: account_variant.is_stream,
        has_prompt_cache_key: account_variant.has_prompt_cache_key,
        request_shape: account_variant.request_shape.clone(),
        protocol_type: account_variant.protocol_type.clone(),
        response_adapter: account_variant.response_adapter,
        gemini_stream_output_mode: account_variant.gemini_stream_output_mode,
        tool_name_restore_map: account_variant.tool_name_restore_map.clone(),
        request_method,
        key_id: api_key.id,
        platform_key_hash: api_key.key_hash,
        local_conversation_id: initial_local_conversation_id,
        conversation_binding: account_variant.conversation_binding.clone(),
        rotation_strategy: api_key.rotation_strategy,
        aggregate_api_id: account_variant.aggregate_api_id.clone(),
        account_plan_filter: api_key.account_plan_filter,
        model_for_log: account_variant.model_for_log.clone(),
        reasoning_for_log: account_variant.reasoning_for_log.clone(),
        service_tier_for_log: account_variant.service_tier_for_log.clone(),
        effective_service_tier_for_log: account_variant
            .effective_service_tier_for_log
            .clone(),
        account_route_variant: if global_channel_priority_enabled {
            Some(account_variant.clone())
        } else {
            None
        },
        aggregate_route_variant,
        method,
    })
}

#[cfg(test)]
#[path = "tests/request_tests.rs"]
mod tests;
