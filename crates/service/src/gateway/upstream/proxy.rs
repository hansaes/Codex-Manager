use crate::apikey_profile::PROTOCOL_ANTHROPIC_NATIVE;
use crate::apikey_profile::ROTATION_AGGREGATE_API;
use crate::gateway::request_log::RequestLogUsage;
use std::time::Instant;
use tiny_http::Request;

use super::super::local_validation::LocalValidationResult;
use super::proxy_pipeline::candidate_executor::{
    execute_candidate_sequence, CandidateExecutionResult, CandidateExecutorParams,
};
use super::proxy_pipeline::execution_context::GatewayUpstreamExecutionContext;
use super::proxy_pipeline::request_gate::acquire_request_gate;
use super::proxy_pipeline::request_setup::prepare_request_setup;
use super::proxy_pipeline::response_finalize::respond_terminal;
use super::support::precheck::{prepare_candidates_for_proxy, CandidatePrecheckResult};

/// 函数 `exhausted_gateway_error_for_log`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - attempted_account_ids: 参数 attempted_account_ids
/// - skipped_cooldown: 参数 skipped_cooldown
/// - skipped_inflight: 参数 skipped_inflight
/// - last_attempt_error: 参数 last_attempt_error
///
/// # 返回
/// 返回函数执行结果
fn exhausted_gateway_error_for_log(
    attempted_account_ids: &[String],
    skipped_cooldown: usize,
    skipped_inflight: usize,
    last_attempt_error: Option<&str>,
) -> String {
    let kind = if !attempted_account_ids.is_empty() {
        "no_available_account_exhausted"
    } else if skipped_cooldown > 0 && skipped_inflight > 0 {
        "no_available_account_skipped"
    } else if skipped_cooldown > 0 {
        "no_available_account_cooldown"
    } else if skipped_inflight > 0 {
        "no_available_account_inflight"
    } else {
        "no_available_account"
    };
    let mut parts = vec![
        crate::gateway::bilingual_error("无可用账号", "no available account"),
        format!("kind={kind}"),
    ];
    if !attempted_account_ids.is_empty() {
        parts.push(format!("attempted={}", attempted_account_ids.join(",")));
    }
    if skipped_cooldown > 0 || skipped_inflight > 0 {
        parts.push(format!(
            "skipped(cooldown={}, inflight={})",
            skipped_cooldown, skipped_inflight
        ));
    }
    if let Some(last_attempt_error) = last_attempt_error
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("last_attempt={last_attempt_error}"));
    }
    parts.join("; ")
}

fn resolve_upstream_is_stream(client_is_stream: bool, path: &str) -> bool {
    let is_compact_path =
        path == "/v1/responses/compact" || path.starts_with("/v1/responses/compact?");
    client_is_stream || (path.starts_with("/v1/responses") && !is_compact_path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewayRouteFamily {
    Account,
    AggregateApi,
}

enum GatewayFamilyOutcome {
    Handled,
    Failed {
        request: Request,
        status_code: u16,
        message: String,
    },
}

fn route_family_for_rotation_strategy(rotation_strategy: &str) -> GatewayRouteFamily {
    if rotation_strategy == ROTATION_AGGREGATE_API {
        GatewayRouteFamily::AggregateApi
    } else {
        GatewayRouteFamily::Account
    }
}

fn resolve_route_family_sequence(
    global_enabled: bool,
    global_order: &str,
    rotation_strategy: &str,
) -> Vec<GatewayRouteFamily> {
    if !global_enabled {
        return vec![route_family_for_rotation_strategy(rotation_strategy)];
    }
    match global_order.trim().to_ascii_lowercase().as_str() {
        "aggregate_first" => vec![GatewayRouteFamily::AggregateApi, GatewayRouteFamily::Account],
        _ => vec![GatewayRouteFamily::Account, GatewayRouteFamily::AggregateApi],
    }
}

fn should_failover_across_route_families(status_code: u16, message: &str) -> bool {
    if matches!(status_code, 401 | 403 | 404 | 408 | 409 | 429 | 500..=599) {
        return true;
    }
    matches!(
        crate::error_codes::classify_message(message),
        crate::error_codes::ErrorCode::BackendProxyError
            | crate::error_codes::ErrorCode::UpstreamTimeout
            | crate::error_codes::ErrorCode::UpstreamChallengeBlocked
            | crate::error_codes::ErrorCode::UpstreamRateLimited
            | crate::error_codes::ErrorCode::UpstreamNotFound
            | crate::error_codes::ErrorCode::UpstreamNonSuccess
            | crate::error_codes::ErrorCode::NoAvailableAccount
            | crate::error_codes::ErrorCode::StreamInterrupted
    )
}

fn route_variant_for_family(
    family: GatewayRouteFamily,
    rotation_strategy: &str,
    default_variant: &super::super::local_validation::GatewayRouteVariant,
    account_route_variant: Option<&super::super::local_validation::GatewayRouteVariant>,
    aggregate_route_variant: Option<&super::super::local_validation::GatewayRouteVariant>,
) -> Option<super::super::local_validation::GatewayRouteVariant> {
    match family {
        GatewayRouteFamily::Account => account_route_variant.cloned().or_else(|| {
            (route_family_for_rotation_strategy(rotation_strategy) == GatewayRouteFamily::Account)
                .then(|| default_variant.clone())
        }),
        GatewayRouteFamily::AggregateApi => aggregate_route_variant.cloned().or_else(|| {
            (route_family_for_rotation_strategy(rotation_strategy)
                == GatewayRouteFamily::AggregateApi)
            .then(|| default_variant.clone())
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_aggregate_route_family(
    request: Request,
    storage: &crate::storage_helpers::StorageHandle,
    trace_id: &str,
    key_id: &str,
    original_path: &str,
    request_method: &str,
    method: &reqwest::Method,
    request_deadline: Option<Instant>,
    started_at: Instant,
    variant: &super::super::local_validation::GatewayRouteVariant,
) -> Result<GatewayFamilyOutcome, String> {
    let path = variant.path.as_str();
    let mut aggregate_api_candidates =
        match super::protocol::aggregate_api::resolve_aggregate_api_rotation_candidates(
            storage,
            variant.protocol_type.as_str(),
            variant.aggregate_api_id.as_deref(),
        ) {
            Ok(candidates) => candidates,
            Err(err) => {
                let message = crate::gateway::bilingual_error("未找到可用聚合 API", err);
                super::super::record_gateway_request_outcome(path, 404, Some("aggregate_api"));
                super::super::trace_log::log_request_final(
                    trace_id,
                    404,
                    Some(key_id),
                    None,
                    Some(message.as_str()),
                    started_at.elapsed().as_millis(),
                );
                super::super::write_request_log(
                    storage,
                    super::super::request_log::RequestLogTraceContext {
                        trace_id: Some(trace_id),
                        original_path: Some(original_path),
                        adapted_path: Some(path),
                        response_adapter: Some(variant.response_adapter),
                        effective_service_tier: variant.effective_service_tier_for_log.as_deref(),
                        ..Default::default()
                    },
                    Some(key_id),
                    None,
                    path,
                    request_method,
                    variant.model_for_log.as_deref(),
                    variant.reasoning_for_log.as_deref(),
                    None,
                    Some(404),
                    super::super::request_log::RequestLogUsage::default(),
                    Some(message.as_str()),
                    Some(started_at.elapsed().as_millis()),
                );
                return Ok(GatewayFamilyOutcome::Failed {
                    request,
                    status_code: 404,
                    message,
                });
            }
        };

    aggregate_api_candidates = super::protocol::aggregate_api::filter_aggregate_api_candidates_by_model(
        storage,
        aggregate_api_candidates,
        variant.model_for_log.as_deref(),
    )?;

    if aggregate_api_candidates.is_empty() {
        let message = match variant
            .model_for_log
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(model) => crate::gateway::bilingual_error(
                "请求模型不在聚合 API 已选择目录中",
                format!("aggregate api model not found in selected catalog: {model}"),
            ),
            None => crate::gateway::bilingual_error("未找到可用聚合 API", "aggregate api not found"),
        };
        super::super::record_gateway_request_outcome(path, 404, Some("aggregate_api"));
        super::super::trace_log::log_request_final(
            trace_id,
            404,
            Some(key_id),
            None,
            Some(message.as_str()),
            started_at.elapsed().as_millis(),
        );
        return Ok(GatewayFamilyOutcome::Failed {
            request,
            status_code: 404,
            message,
        });
    }

    super::protocol::aggregate_api::apply_gateway_route_strategy_to_aggregate_candidates(
        &mut aggregate_api_candidates,
        key_id,
        variant.model_for_log.as_deref(),
        variant.aggregate_api_id.as_deref(),
    );

    match super::protocol::aggregate_api::proxy_aggregate_request(
        super::protocol::aggregate_api::AggregateProxyRequest {
            request,
            storage,
            trace_id,
            key_id,
            original_path,
            path,
            request_method,
            method,
            body: &variant.body,
            is_stream: variant.is_stream,
            response_adapter: variant.response_adapter,
            model_for_log: variant.model_for_log.as_deref(),
            reasoning_for_log: variant.reasoning_for_log.as_deref(),
            effective_service_tier_for_log: variant.effective_service_tier_for_log.as_deref(),
            aggregate_api_candidates,
            request_deadline,
            started_at,
        },
    )? {
        super::protocol::aggregate_api::AggregateProxyOutcome::Handled => {
            Ok(GatewayFamilyOutcome::Handled)
        }
        super::protocol::aggregate_api::AggregateProxyOutcome::Failed {
            request,
            status_code,
            message,
        } => Ok(GatewayFamilyOutcome::Failed {
            request,
            status_code,
            message,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_account_route_family(
    request: Request,
    storage: &crate::storage_helpers::StorageHandle,
    trace_id: &str,
    key_id: &str,
    original_path: &str,
    request_method: &str,
    platform_key_hash: &str,
    local_conversation_id: Option<&str>,
    account_plan_filter: Option<&str>,
    method: &reqwest::Method,
    request_deadline: Option<Instant>,
    started_at: Instant,
    debug: bool,
    variant: &super::super::local_validation::GatewayRouteVariant,
) -> Result<GatewayFamilyOutcome, String> {
    let path = variant.path.as_str();
    let client_is_stream = variant.is_stream;
    let upstream_is_stream = resolve_upstream_is_stream(client_is_stream, path);
    let (request, mut candidates) = match prepare_candidates_for_proxy(
        request,
        storage,
        trace_id,
        key_id,
        original_path,
        path,
        variant.response_adapter,
        request_method,
        variant.model_for_log.as_deref(),
        variant.reasoning_for_log.as_deref(),
        account_plan_filter,
    ) {
        CandidatePrecheckResult::Ready {
            request,
            candidates,
        } => (request, candidates),
        CandidatePrecheckResult::Failed {
            request,
            status_code,
            message,
        } => {
            return Ok(GatewayFamilyOutcome::Failed {
                request,
                status_code,
                message,
            });
        }
    };
    let setup = prepare_request_setup(
        path,
        variant.protocol_type.as_str(),
        variant.has_prompt_cache_key,
        &variant.incoming_headers,
        &variant.body,
        &mut candidates,
        key_id,
        platform_key_hash,
        local_conversation_id,
        variant.conversation_binding.as_ref(),
        variant.model_for_log.as_deref(),
        trace_id,
    );
    let base = setup.upstream_base.clone();
    let context = GatewayUpstreamExecutionContext::new(
        trace_id,
        storage,
        key_id,
        original_path,
        path,
        request_method,
        variant.response_adapter,
        variant.protocol_type.as_str(),
        variant.model_for_log.as_deref(),
        variant.reasoning_for_log.as_deref(),
        variant.service_tier_for_log.as_deref(),
        variant.effective_service_tier_for_log.as_deref(),
        setup.candidate_count,
        setup.account_max_inflight,
    );
    let allow_openai_fallback = setup.upstream_fallback_base.is_some();
    let disable_challenge_stateless_retry = !(variant.protocol_type == PROTOCOL_ANTHROPIC_NATIVE
        && variant.body.len() <= 2 * 1024)
        && !path.starts_with("/v1/responses");
    let _request_gate_guard = acquire_request_gate(
        trace_id,
        key_id,
        path,
        variant.model_for_log.as_deref(),
        request_deadline,
    );
    match execute_candidate_sequence(
        request,
        candidates,
        CandidateExecutorParams {
            storage,
            method,
            incoming_headers: &variant.incoming_headers,
            body: &variant.body,
            path,
            request_shape: variant.request_shape.as_deref(),
            trace_id,
            model_for_log: variant.model_for_log.as_deref(),
            response_adapter: variant.response_adapter,
            gemini_stream_output_mode: variant.gemini_stream_output_mode,
            tool_name_restore_map: &variant.tool_name_restore_map,
            context: &context,
            setup: &setup,
            request_deadline,
            started_at,
            client_is_stream,
            upstream_is_stream,
            debug,
            allow_openai_fallback,
            disable_challenge_stateless_retry,
        },
    )? {
        CandidateExecutionResult::Handled => Ok(GatewayFamilyOutcome::Handled),
        CandidateExecutionResult::TerminalFailure {
            request,
            status_code,
            message,
            attempted_account_ids,
            last_attempt_url,
            model_for_log,
        } => {
            context.log_final_result_with_model(
                None,
                last_attempt_url.as_deref().or(Some(base.as_str())),
                model_for_log.as_deref(),
                status_code,
                RequestLogUsage::default(),
                Some(message.as_str()),
                started_at.elapsed().as_millis(),
                (!attempted_account_ids.is_empty()).then_some(attempted_account_ids.as_slice()),
            );
            Ok(GatewayFamilyOutcome::Failed {
                request,
                status_code,
                message,
            })
        }
        CandidateExecutionResult::Exhausted {
            request,
            attempted_account_ids,
            skipped_cooldown,
            skipped_inflight,
            last_attempt_url,
            last_attempt_error,
            final_status_code,
        } => {
            let log_message = exhausted_gateway_error_for_log(
                attempted_account_ids.as_slice(),
                skipped_cooldown,
                skipped_inflight,
                last_attempt_error.as_deref(),
            );
            let status_code = if last_attempt_error.is_some() {
                final_status_code
            } else {
                503
            };
            let message = last_attempt_error.unwrap_or_else(|| {
                crate::gateway::bilingual_error("无可用账号", "no available account")
            });
            context.log_final_result(
                None,
                last_attempt_url.as_deref().or(Some(base.as_str())),
                status_code,
                RequestLogUsage::default(),
                Some(log_message.as_str()),
                started_at.elapsed().as_millis(),
                (!attempted_account_ids.is_empty()).then_some(attempted_account_ids.as_slice()),
            );
            Ok(GatewayFamilyOutcome::Failed {
                request,
                status_code,
                message,
            })
        }
    }
}

/// 函数 `proxy_validated_request`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - in super: 参数 in super
///
/// # 返回
/// 返回函数执行结果
pub(in super::super) fn proxy_validated_request(
    request: Request,
    validated: LocalValidationResult,
    debug: bool,
) -> Result<(), String> {
    let LocalValidationResult {
        trace_id,
        incoming_headers,
        storage,
        original_path,
        path,
        body,
        is_stream,
        has_prompt_cache_key,
        request_shape,
        protocol_type,
        rotation_strategy,
        aggregate_api_id,
        account_plan_filter,
        response_adapter,
        gemini_stream_output_mode,
        tool_name_restore_map,
        request_method,
        key_id,
        platform_key_hash,
        local_conversation_id,
        conversation_binding,
        model_for_log,
        reasoning_for_log,
        service_tier_for_log,
        effective_service_tier_for_log,
        account_route_variant,
        aggregate_route_variant,
        method,
    } = validated;
    let started_at = Instant::now();
    let client_is_stream = is_stream;
    let request_deadline = super::support::deadline::request_deadline(started_at, client_is_stream);

    super::super::trace_log::log_request_start(
        trace_id.as_str(),
        key_id.as_str(),
        request_method.as_str(),
        path.as_str(),
        model_for_log.as_deref(),
        reasoning_for_log.as_deref(),
        service_tier_for_log.as_deref(),
        client_is_stream,
        "http",
        protocol_type.as_str(),
    );
    super::super::trace_log::log_request_body_preview(trace_id.as_str(), body.as_ref());
    if protocol_type == crate::apikey_profile::PROTOCOL_GEMINI_NATIVE {
        super::super::trace_log::log_gemini_request_diagnostics(
            trace_id.as_str(),
            original_path.as_str(),
            path.as_str(),
            format!("{response_adapter:?}").as_str(),
            gemini_stream_output_mode.map(|mode| match mode {
                super::super::GeminiStreamOutputMode::Sse => "sse",
                super::super::GeminiStreamOutputMode::Raw => "raw",
            }),
            body.as_ref(),
        );
    }

    let default_route_variant = super::super::local_validation::GatewayRouteVariant {
        incoming_headers: incoming_headers.clone(),
        path: path.clone(),
        body: body.clone(),
        is_stream: client_is_stream,
        has_prompt_cache_key,
        request_shape: request_shape.clone(),
        protocol_type: protocol_type.clone(),
        aggregate_api_id: aggregate_api_id.clone(),
        response_adapter,
        gemini_stream_output_mode,
        tool_name_restore_map: tool_name_restore_map.clone(),
        conversation_binding: conversation_binding.clone(),
        model_for_log: model_for_log.clone(),
        reasoning_for_log: reasoning_for_log.clone(),
        service_tier_for_log: service_tier_for_log.clone(),
        effective_service_tier_for_log: effective_service_tier_for_log.clone(),
    };
    let family_sequence = resolve_route_family_sequence(
        crate::gateway::global_channel_priority_enabled(),
        crate::gateway::current_global_channel_priority_order(),
        rotation_strategy.as_str(),
    );
    let mut request = request;
    for (family_index, family) in family_sequence.iter().enumerate() {
        let variant = match route_variant_for_family(
            *family,
            rotation_strategy.as_str(),
            &default_route_variant,
            account_route_variant.as_ref(),
            aggregate_route_variant.as_ref(),
        ) {
            Some(variant) => variant,
            None => {
                if family_index + 1 < family_sequence.len() {
                    continue;
                }
                let message = match family {
                    GatewayRouteFamily::Account => crate::gateway::bilingual_error(
                        "当前请求无法构建账号通道",
                        "account route variant unavailable",
                    ),
                    GatewayRouteFamily::AggregateApi => crate::gateway::bilingual_error(
                        "当前请求无法构建聚合 API 通道",
                        "aggregate api route variant unavailable",
                    ),
                };
                return respond_terminal(request, 503, message, Some(trace_id.as_str()));
            }
        };

        let outcome = match family {
            GatewayRouteFamily::Account => run_account_route_family(
                request,
                &storage,
                trace_id.as_str(),
                key_id.as_str(),
                original_path.as_str(),
                request_method.as_str(),
                platform_key_hash.as_str(),
                local_conversation_id.as_deref(),
                account_plan_filter.as_deref(),
                &method,
                request_deadline,
                started_at,
                debug,
                &variant,
            )?,
            GatewayRouteFamily::AggregateApi => run_aggregate_route_family(
                request,
                &storage,
                trace_id.as_str(),
                key_id.as_str(),
                original_path.as_str(),
                request_method.as_str(),
                &method,
                request_deadline,
                started_at,
                &variant,
            )?,
        };

        match outcome {
            GatewayFamilyOutcome::Handled => return Ok(()),
            GatewayFamilyOutcome::Failed {
                request: failed_request,
                status_code,
                message,
            } => {
                if family_index + 1 < family_sequence.len()
                    && should_failover_across_route_families(status_code, message.as_str())
                {
                    request = failed_request;
                    continue;
                }
                return respond_terminal(
                    failed_request,
                    status_code,
                    message,
                    Some(trace_id.as_str()),
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        exhausted_gateway_error_for_log, resolve_route_family_sequence,
        resolve_upstream_is_stream, should_failover_across_route_families, GatewayRouteFamily,
    };

    /// 函数 `exhausted_gateway_error_includes_attempts_skips_and_last_error`
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
    fn exhausted_gateway_error_includes_attempts_skips_and_last_error() {
        let message = exhausted_gateway_error_for_log(
            &["acc-a".to_string(), "acc-b".to_string()],
            2,
            1,
            Some("upstream challenge blocked"),
        );

        assert!(message.contains("no available account"));
        assert!(message.contains("kind=no_available_account_exhausted"));
        assert!(message.contains("attempted=acc-a,acc-b"));
        assert!(message.contains("skipped(cooldown=2, inflight=1)"));
        assert!(message.contains("last_attempt=upstream challenge blocked"));
    }

    /// 函数 `exhausted_gateway_error_marks_cooldown_only_skip_kind`
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
    fn exhausted_gateway_error_marks_cooldown_only_skip_kind() {
        let message = exhausted_gateway_error_for_log(&[], 2, 0, None);

        assert!(message.contains("kind=no_available_account_cooldown"));
    }

    #[test]
    fn resolve_upstream_is_stream_keeps_non_compact_responses_on_sse_upstream() {
        assert!(resolve_upstream_is_stream(false, "/v1/responses"));
        assert!(resolve_upstream_is_stream(
            false,
            "/v1/responses?stream=false"
        ));
        assert!(!resolve_upstream_is_stream(false, "/v1/responses/compact"));
        assert!(!resolve_upstream_is_stream(false, "/v1/chat/completions"));
        assert!(resolve_upstream_is_stream(true, "/v1/chat/completions"));
    }

    #[test]
    fn resolve_route_family_sequence_prefers_global_account_first_order() {
        assert_eq!(
            resolve_route_family_sequence(
                true,
                "account_first",
                crate::apikey_profile::ROTATION_AGGREGATE_API,
            ),
            vec![GatewayRouteFamily::Account, GatewayRouteFamily::AggregateApi]
        );
    }

    #[test]
    fn resolve_route_family_sequence_prefers_global_aggregate_first_order() {
        assert_eq!(
            resolve_route_family_sequence(
                true,
                "aggregate_first",
                crate::apikey_profile::ROTATION_ACCOUNT,
            ),
            vec![GatewayRouteFamily::AggregateApi, GatewayRouteFamily::Account]
        );
    }

    #[test]
    fn resolve_route_family_sequence_keeps_legacy_strategy_when_global_priority_disabled() {
        assert_eq!(
            resolve_route_family_sequence(
                false,
                "aggregate_first",
                crate::apikey_profile::ROTATION_AGGREGATE_API,
            ),
            vec![GatewayRouteFamily::AggregateApi]
        );
        assert_eq!(
            resolve_route_family_sequence(
                false,
                "aggregate_first",
                crate::apikey_profile::ROTATION_ACCOUNT,
            ),
            vec![GatewayRouteFamily::Account]
        );
    }

    #[test]
    fn should_failover_across_route_families_for_runtime_failures() {
        assert!(should_failover_across_route_families(
            401,
            "aggregate api upstream status=401"
        ));
        assert!(should_failover_across_route_families(
            429,
            "type=rate_limit_error Too many requests"
        ));
        assert!(should_failover_across_route_families(
            502,
            "aggregate api upstream status=502"
        ));
        assert!(should_failover_across_route_families(
            200,
            "aggregate api request timeout"
        ));
    }

    #[test]
    fn should_not_failover_across_route_families_for_client_side_validation_errors() {
        assert!(!should_failover_across_route_families(
            400,
            "invalid aggregate api authParams"
        ));
        assert!(!should_failover_across_route_families(
            422,
            "invalid request payload"
        ));
    }
}
