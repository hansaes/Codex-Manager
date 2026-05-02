use codexmanager_core::rpc::types::ModelsResponse;
use serde_json::json;
const MODEL_CACHE_SCOPE_DEFAULT: &str = "default";
const OPENAI_MODELS_LIST_OBJECT: &str = "list";
const OPENAI_MODEL_OBJECT: &str = "model";
const DEFAULT_MODEL_CREATED_TS: i64 = 1_626_777_600;

/// 函数 `serialize_models_response`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-12
///
/// # 参数
/// - models: 参数 models
///
/// # 返回
/// 返回函数执行结果
fn serialize_models_response(models: &ModelsResponse) -> String {
    let models_value = serde_json::to_value(models).unwrap_or_else(|_| json!({ "models": [] }));
    let model_items = models
        .models
        .iter()
        .map(openai_model_item_from_model_info)
        .collect::<Vec<_>>();
    let mut root = match models_value {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    root.insert("data".to_string(), serde_json::Value::Array(model_items));
    root.insert(
        "object".to_string(),
        serde_json::Value::String(OPENAI_MODELS_LIST_OBJECT.to_string()),
    );
    root.insert("success".to_string(), serde_json::Value::Bool(true));
    serde_json::to_string(&serde_json::Value::Object(root)).unwrap_or_else(|_| {
        "{\"models\":[],\"data\":[],\"object\":\"list\",\"success\":true}".to_string()
    })
}

fn openai_model_item_from_model_info(
    model: &codexmanager_core::rpc::types::ModelInfo,
) -> serde_json::Value {
    let owned_by = model
        .extra
        .get("owned_by")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("custom");
    let created = model
        .extra
        .get("created")
        .and_then(|value| value.as_i64())
        .unwrap_or(DEFAULT_MODEL_CREATED_TS);
    let supported_endpoint_types = model
        .extra
        .get("supported_endpoint_types")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim))
                .filter(|value| !value.is_empty())
                .map(|value| serde_json::Value::String(value.to_string()))
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![serde_json::Value::String("openai".to_string())]);

    json!({
        "id": model.slug,
        "object": OPENAI_MODEL_OBJECT,
        "created": created,
        "owned_by": owned_by,
        "supported_endpoint_types": supported_endpoint_types,
    })
}

fn should_hide_model_descriptions_for_request(request: &tiny_http::Request) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("User-Agent")
            && header
                .value
                .as_str()
                .to_ascii_lowercase()
                .contains("codex_cli_rs")
    })
}

fn response_models_for_client(models: &ModelsResponse, hide_descriptions: bool) -> ModelsResponse {
    if !hide_descriptions {
        return models.clone();
    }

    let mut response = models.clone();
    for model in &mut response.models {
        model.description = None;
    }
    response
}

/// 函数 `read_cached_models_response`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-12
///
/// # 参数
/// - storage: 参数 storage
///
/// # 返回
/// 返回函数执行结果
fn read_cached_models_response(
    storage: &codexmanager_core::storage::Storage,
) -> Result<ModelsResponse, String> {
    crate::apikey_models::read_model_options_from_storage(storage)
}

fn aggregate_models_response_for_key(
    storage: &codexmanager_core::storage::Storage,
    key_id: &str,
) -> Result<Option<ModelsResponse>, String> {
    let Some(api_key) = storage
        .find_api_key_by_id(key_id)
        .map_err(|err| err.to_string())?
    else {
        return Ok(None);
    };
    if api_key.rotation_strategy != crate::apikey_profile::ROTATION_AGGREGATE_API {
        if !crate::gateway::global_channel_priority_enabled() {
            return Ok(None);
        }
        let candidates = match crate::gateway::upstream::protocol::aggregate_api::resolve_aggregate_api_rotation_candidates(
            storage,
            api_key.protocol_type.as_str(),
            None,
        ) {
            Ok(items) => items,
            Err(_) => return Ok(None),
        };
        let mut seen = std::collections::HashSet::new();
        let mut models = Vec::new();
        for candidate in candidates {
            for item in storage
                .list_aggregate_api_models(candidate.id.as_str())
                .map_err(|err| err.to_string())?
            {
                let slug = item.model_slug.trim().to_ascii_lowercase();
                if slug.is_empty() || !seen.insert(slug) {
                    continue;
                }
                models.push(codexmanager_core::rpc::types::ModelInfo {
                    slug: item.model_slug.clone(),
                    display_name: item.display_name.unwrap_or_else(|| item.model_slug.clone()),
                    supported_in_api: true,
                    visibility: Some("list".to_string()),
                    ..Default::default()
                });
            }
        }
        if models.is_empty() {
            return Ok(None);
        }
        return Ok(Some(ModelsResponse {
            models,
            ..Default::default()
        }));
    }
    let Some(resolved_catalog) =
        super::aggregate_catalog::resolve_aggregate_model_catalog(storage, &api_key, None)?
    else {
        return Ok(None);
    };
    Ok(Some(ModelsResponse {
        models: resolved_catalog
            .models
            .into_iter()
            .map(|item| codexmanager_core::rpc::types::ModelInfo {
                slug: item.model_slug.clone(),
                display_name: item.display_name.unwrap_or_else(|| item.model_slug.clone()),
                supported_in_api: true,
                visibility: Some("list".to_string()),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }))
}

/// 函数 `maybe_respond_local_models`
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
pub(super) fn maybe_respond_local_models(
    request: tiny_http::Request,
    trace_id: &str,
    key_id: &str,
    protocol_type: &str,
    original_path: &str,
    path: &str,
    response_adapter: super::ResponseAdapter,
    request_method: &str,
    model_for_log: Option<&str>,
    reasoning_for_log: Option<&str>,
    storage: &codexmanager_core::storage::Storage,
) -> Result<Option<tiny_http::Request>, String> {
    let is_models_list = request_method.eq_ignore_ascii_case("GET")
        && (path == "/v1/models" || path.starts_with("/v1/models?"));
    if !is_models_list {
        return Ok(Some(request));
    }
    let context = super::local_response::LocalResponseContext {
        trace_id,
        key_id,
        protocol_type,
        original_path,
        path,
        response_adapter,
        request_method,
        model_for_log,
        reasoning_for_log,
        storage,
    };
    let hide_descriptions = should_hide_model_descriptions_for_request(&request);

    let aggregate_models = match aggregate_models_response_for_key(storage, key_id) {
        Ok(models) => models,
        Err(err) => {
            let message = crate::gateway::bilingual_error(
                "读取聚合模型失败",
                format!("aggregate model catalog read failed: {err}"),
            );
            super::local_response::respond_local_terminal_error(request, &context, 503, message)?;
            return Ok(None);
        }
    };

    let cached = match read_cached_models_response(storage) {
        Ok(models) => models,
        Err(err) => {
            let message = crate::gateway::bilingual_error(
                "读取模型缓存失败",
                format!("model options cache read failed: {err}"),
            );
            super::local_response::respond_local_terminal_error(request, &context, 503, message)?;
            return Ok(None);
        }
    };

    let models = if crate::gateway::global_channel_priority_enabled() {
        match (cached.is_empty(), aggregate_models) {
            (true, Some(aggregate_models)) => aggregate_models,
            (false, Some(aggregate_models)) => {
                crate::apikey_models::merge_models_response(cached.clone(), aggregate_models)
            }
            (false, None) => cached,
            (true, None) => match super::fetch_models_for_picker() {
                Ok(fetched) if !fetched.is_empty() => {
                    let merged =
                        crate::apikey_models::merge_models_response(cached.clone(), fetched);
                    if let Err(err) =
                        crate::apikey_models::save_model_options_with_storage(storage, &merged)
                    {
                        log::warn!(
                            "event=gateway_model_catalog_upsert_failed scope={} err={}",
                            MODEL_CACHE_SCOPE_DEFAULT,
                            err
                        );
                    }
                    merged
                }
                Ok(_) => {
                    let message = crate::gateway::bilingual_error(
                        "模型刷新后返回空目录",
                        "models refresh returned empty catalog",
                    );
                    super::local_response::respond_local_terminal_error(
                        request, &context, 503, message,
                    )?;
                    return Ok(None);
                }
                Err(err) => {
                    let message = crate::gateway::bilingual_error(
                        "模型刷新失败",
                        format!("models refresh failed: {err}"),
                    );
                    super::local_response::respond_local_terminal_error(
                        request, &context, 503, message,
                    )?;
                    return Ok(None);
                }
            },
        }
    } else if let Some(aggregate_models) = aggregate_models {
        aggregate_models
    } else if !cached.is_empty() {
        cached
    } else {
        match super::fetch_models_for_picker() {
            Ok(fetched) if !fetched.is_empty() => {
                let merged = crate::apikey_models::merge_models_response(cached.clone(), fetched);
                if let Err(err) =
                    crate::apikey_models::save_model_options_with_storage(storage, &merged)
                {
                    log::warn!(
                        "event=gateway_model_catalog_upsert_failed scope={} err={}",
                        MODEL_CACHE_SCOPE_DEFAULT,
                        err
                    );
                }
                merged
            }
            Ok(_) => {
                let message = crate::gateway::bilingual_error(
                    "模型刷新后返回空目录",
                    "models refresh returned empty catalog",
                );
                super::local_response::respond_local_terminal_error(
                    request, &context, 503, message,
                )?;
                return Ok(None);
            }
            Err(err) => {
                let message = crate::gateway::bilingual_error(
                    "模型刷新失败",
                    format!("models refresh failed: {err}"),
                );
                super::local_response::respond_local_terminal_error(
                    request, &context, 503, message,
                )?;
                return Ok(None);
            }
        }
    };

    let response_models = response_models_for_client(&models, hide_descriptions);
    let output = serialize_models_response(&response_models);
    super::local_response::respond_local_json(
        request,
        &context,
        output,
        super::request_log::RequestLogUsage::default(),
    )?;
    Ok(None)
}

#[cfg(test)]
#[path = "tests/local_models_tests.rs"]
mod tests;
