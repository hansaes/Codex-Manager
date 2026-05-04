use serde_json::{json, Map, Value};

use super::tool_mapping::restore_openai_tool_name;
use super::ToolNameRestoreMap;

fn parse_json_value(body: &[u8], err: &str) -> Result<Value, String> {
    serde_json::from_slice(body).map_err(|_| err.to_string())
}

/// 函数 `map_openai_error_to_anthropic`
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
fn map_openai_error_to_anthropic(value: &Value) -> Option<Value> {
    let error = value.get("error")?.as_object()?;
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("upstream request failed")
        .to_string();
    let error_type = error
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("api_error");

    let mapped_error_type = match error_type {
        "authentication_error" => "authentication_error",
        "permission_error" => "permission_error",
        "rate_limit_error" => "rate_limit_error",
        "invalid_request_error" | "not_found_error" => "invalid_request_error",
        _ => "api_error",
    };

    Some(json!({
        "type": "error",
        "error": {
            "type": mapped_error_type,
            "message": message,
        }
    }))
}

/// 函数 `convert_openai_json_to_anthropic`
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
pub(super) fn convert_openai_json_to_anthropic(
    body: &[u8],
    tool_name_restore_map: Option<&ToolNameRestoreMap>,
) -> Result<(Vec<u8>, &'static str), String> {
    let value = parse_json_value(body, "invalid upstream json response")?;
    if let Some(error_payload) = map_openai_error_to_anthropic(&value) {
        return serde_json::to_vec(&error_payload)
            .map(|bytes| (bytes, "application/json"))
            .map_err(|err| format!("serialize claude error response failed: {err}"));
    }

    let payload = if value.get("choices").is_some() {
        build_anthropic_message_from_chat_completions(&value, tool_name_restore_map)?
    } else {
        build_anthropic_message_from_responses(&value, tool_name_restore_map)?
    };

    serde_json::to_vec(&payload)
        .map(|bytes| (bytes, "application/json"))
        .map_err(|err| format!("serialize claude response failed: {err}"))
}

fn map_openai_error_to_responses(value: &Value) -> Option<Value> {
    let error = value.get("error")?.as_object()?;
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("upstream request failed");
    let error_type = error
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("server_error");
    Some(json!({
        "error": {
            "message": message,
            "type": error_type,
        }
    }))
}

fn normalize_response_usage_from_chat(usage: Option<&Map<String, Value>>) -> Option<Value> {
    let usage = usage?;
    let input_tokens = extract_usage_i64(Some(usage), &["input_tokens", "prompt_tokens"]);
    let output_tokens = extract_usage_i64(Some(usage), &["output_tokens", "completion_tokens"]);
    let total_tokens = extract_usage_i64(Some(usage), &["total_tokens"]);
    let cached_tokens = extract_usage_i64(
        Some(usage),
        &["input_tokens_details.cached_tokens", "prompt_tokens_details.cached_tokens"],
    );
    let reasoning_tokens = extract_usage_i64(
        Some(usage),
        &["output_tokens_details.reasoning_tokens", "completion_tokens_details.reasoning_tokens"],
    );

    let mut out = Map::new();
    if let Some(value) = input_tokens {
        out.insert("input_tokens".to_string(), Value::from(value));
    }
    if let Some(value) = output_tokens {
        out.insert("output_tokens".to_string(), Value::from(value));
    }
    if let Some(value) = total_tokens {
        out.insert("total_tokens".to_string(), Value::from(value));
    }
    if let Some(value) = cached_tokens {
        out.insert(
            "input_tokens_details".to_string(),
            json!({ "cached_tokens": value }),
        );
    }
    if let Some(value) = reasoning_tokens {
        out.insert(
            "output_tokens_details".to_string(),
            json!({ "reasoning_tokens": value }),
        );
    }

    if out.is_empty() {
        None
    } else {
        Some(Value::Object(out))
    }
}

fn extract_openai_chat_message_output_items(
    message: Option<&Map<String, Value>>,
    tool_name_restore_map: Option<&ToolNameRestoreMap>,
) -> Vec<Value> {
    let Some(message) = message else {
        return Vec::new();
    };
    let mut output = Vec::new();

    let text = message
        .get("content")
        .map(extract_openai_text_content)
        .transpose()
        .unwrap_or_default()
        .unwrap_or_default();
    if !text.trim().is_empty() {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": text.trim(),
            }]
        }));
    }

    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tool_call in tool_calls {
            let Some(tool_call_obj) = tool_call.as_object() else {
                continue;
            };
            let call_id = tool_call_obj
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("call_{}", output.len()));
            let Some(function_name) = tool_call_obj
                .get("function")
                .and_then(Value::as_object)
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let function_name = restore_openai_tool_name(function_name, tool_name_restore_map);
            let arguments = tool_call_obj
                .get("function")
                .and_then(Value::as_object)
                .and_then(|function| function.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("{}");
            output.push(json!({
                "type": "function_call",
                "call_id": call_id,
                "name": function_name,
                "arguments": arguments,
            }));
        }
    }

    output
}

fn build_responses_output_from_chat_completion(
    value: &Value,
    tool_name_restore_map: Option<&ToolNameRestoreMap>,
) -> Result<Value, String> {
    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| "missing upstream choice".to_string())?;
    let message = choice.get("message").and_then(Value::as_object);
    Ok(Value::Array(extract_openai_chat_message_output_items(
        message,
        tool_name_restore_map,
    )))
}

fn anthropic_content_blocks_to_responses_output(content: Option<&[Value]>) -> Vec<Value> {
    let Some(content) = content else {
        return Vec::new();
    };

    let mut text_parts = Vec::new();
    let mut output = Vec::new();
    for block in content {
        let Some(block_obj) = block.as_object() else {
            continue;
        };
        match block_obj.get("type").and_then(Value::as_str).unwrap_or_default() {
            "text" => {
                if let Some(text) = block_obj.get("text").and_then(Value::as_str) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        text_parts.push(trimmed.to_string());
                    }
                }
            }
            "thinking" => {
                let mut summary = Vec::new();
                if let Some(text) = block_obj.get("thinking").and_then(Value::as_str) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        summary.push(json!({
                            "type": "summary_text",
                            "text": trimmed,
                        }));
                    }
                }
                let mut item = Map::new();
                item.insert("type".to_string(), Value::String("reasoning".to_string()));
                if let Some(id) = block_obj.get("id").and_then(Value::as_str) {
                    item.insert("id".to_string(), Value::String(id.to_string()));
                }
                if !summary.is_empty() {
                    item.insert("summary".to_string(), Value::Array(summary));
                }
                if let Some(signature) = block_obj
                    .get("signature")
                    .or_else(|| block_obj.get("encrypted_content"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    item.insert(
                        "encrypted_content".to_string(),
                        Value::String(signature.to_string()),
                    );
                }
                output.push(Value::Object(item));
            }
            "tool_use" => {
                let Some(name) = block_obj
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let call_id = block_obj
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("call_{}", output.len()));
                let arguments = block_obj
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let arguments = if arguments.is_string() {
                    arguments
                } else {
                    Value::String(
                        serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_string()),
                    )
                };
                output.push(json!({
                    "type": "function_call",
                    "call_id": call_id,
                    "name": name,
                    "arguments": arguments.as_str().unwrap_or("{}"),
                }));
            }
            _ => {}
        }
    }

    if !text_parts.is_empty() {
        output.insert(
            0,
            json!({
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": text_parts.join("\n"),
                }]
            }),
        );
    }

    output
}

fn normalize_response_usage_from_anthropic(usage: Option<&Map<String, Value>>) -> Option<Value> {
    let usage = usage?;
    let mut out = Map::new();

    if let Some(value) = usage.get("input_tokens").and_then(Value::as_i64) {
        out.insert("input_tokens".to_string(), Value::from(value));
    }
    if let Some(value) = usage.get("output_tokens").and_then(Value::as_i64) {
        out.insert("output_tokens".to_string(), Value::from(value));
    }
    if let Some(value) = usage.get("total_tokens").and_then(Value::as_i64) {
        out.insert("total_tokens".to_string(), Value::from(value));
    } else {
        let total = usage.get("input_tokens").and_then(Value::as_i64).unwrap_or(0)
            + usage.get("output_tokens").and_then(Value::as_i64).unwrap_or(0);
        if total > 0 {
            out.insert("total_tokens".to_string(), Value::from(total));
        }
    }
    if let Some(value) = usage.get("cache_read_input_tokens").and_then(Value::as_i64) {
        out.insert(
            "input_tokens_details".to_string(),
            json!({ "cached_tokens": value }),
        );
    }
    if let Some(value) = usage.get("reasoning_output_tokens").and_then(Value::as_i64) {
        out.insert(
            "output_tokens_details".to_string(),
            json!({ "reasoning_tokens": value }),
        );
    }

    if out.is_empty() {
        None
    } else {
        Some(Value::Object(out))
    }
}

pub(super) fn convert_openai_chat_json_to_responses(
    body: &[u8],
    tool_name_restore_map: Option<&ToolNameRestoreMap>,
) -> Result<(Vec<u8>, &'static str), String> {
    let value = parse_json_value(body, "invalid upstream chat json response")?;
    if let Some(error_payload) = map_openai_error_to_responses(&value) {
        return serde_json::to_vec(&error_payload)
            .map(|bytes| (bytes, "application/json"))
            .map_err(|err| format!("serialize responses error response failed: {err}"));
    }

    let output = if value.get("output").is_some() {
        value.get("output").cloned().unwrap_or_else(|| Value::Array(vec![]))
    } else {
        build_responses_output_from_chat_completion(&value, tool_name_restore_map)?
    };

    let output_text = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(Value::as_object)
        .and_then(|message| message.get("content"))
        .map(extract_openai_text_content)
        .transpose()
        .unwrap_or_default()
        .unwrap_or_default();

    let usage = value
        .get("usage")
        .and_then(Value::as_object)
        .and_then(|usage| normalize_response_usage_from_chat(Some(usage)));
    let mut out = Map::new();
    out.insert(
        "id".to_string(),
        value.get("id").cloned().unwrap_or_else(|| Value::String("resp_proxy".to_string())),
    );
    out.insert("object".to_string(), Value::String("response".to_string()));
    out.insert(
        "created".to_string(),
        value.get("created").cloned().unwrap_or_else(|| Value::Number(0.into())),
    );
    out.insert(
        "model".to_string(),
        value
            .get("model")
            .cloned()
            .unwrap_or_else(|| Value::String("unknown".to_string())),
    );
    out.insert("status".to_string(), Value::String("completed".to_string()));
    out.insert("output".to_string(), output);
    if !output_text.trim().is_empty() {
        out.insert(
            "output_text".to_string(),
            Value::String(output_text.trim().to_string()),
        );
    }
    if let Some(usage) = usage {
        out.insert("usage".to_string(), usage);
    }

    serde_json::to_vec(&Value::Object(out))
        .map(|bytes| (bytes, "application/json"))
        .map_err(|err| format!("serialize responses json failed: {err}"))
}

pub(super) fn convert_anthropic_json_to_responses(
    body: &[u8],
) -> Result<(Vec<u8>, &'static str), String> {
    let value = parse_json_value(body, "invalid upstream anthropic json response")?;

    if value.get("error").is_some() {
        let message = value
            .get("error")
            .and_then(Value::as_object)
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("upstream request failed");
        let error_type = value
            .get("error")
            .and_then(Value::as_object)
            .and_then(|error| error.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("server_error");
        let payload = json!({
            "error": {
                "message": message,
                "type": error_type,
            }
        });
        return serde_json::to_vec(&payload)
            .map(|bytes| (bytes, "application/json"))
            .map_err(|err| format!("serialize responses error response failed: {err}"));
    }

    let output = anthropic_content_blocks_to_responses_output(
        value.get("content").and_then(Value::as_array).map(Vec::as_slice),
    );
    let output_text = value
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let usage = value
        .get("usage")
        .and_then(Value::as_object)
        .and_then(|usage| normalize_response_usage_from_anthropic(Some(usage)));

    let mut out = Map::new();
    out.insert(
        "id".to_string(),
        value
            .get("id")
            .cloned()
            .unwrap_or_else(|| Value::String("resp_proxy".to_string())),
    );
    out.insert("object".to_string(), Value::String("response".to_string()));
    out.insert(
        "created".to_string(),
        value.get("created").cloned().unwrap_or_else(|| Value::Number(0.into())),
    );
    out.insert(
        "model".to_string(),
        value
            .get("model")
            .cloned()
            .unwrap_or_else(|| Value::String("unknown".to_string())),
    );
    out.insert("status".to_string(), Value::String("completed".to_string()));
    out.insert("output".to_string(), Value::Array(output));
    if !output_text.trim().is_empty() {
        out.insert(
            "output_text".to_string(),
            Value::String(output_text.trim().to_string()),
        );
    }
    if let Some(usage) = usage {
        out.insert("usage".to_string(), usage);
    }

    serde_json::to_vec(&Value::Object(out))
        .map(|bytes| (bytes, "application/json"))
        .map_err(|err| format!("serialize responses json failed: {err}"))
}

/// 函数 `build_anthropic_message_from_chat_completions`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
/// - tool_name_restore_map: 参数 tool_name_restore_map
///
/// # 返回
/// 返回函数执行结果
fn build_anthropic_message_from_chat_completions(
    value: &Value,
    tool_name_restore_map: Option<&ToolNameRestoreMap>,
) -> Result<Value, String> {
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("msg_codexmanager");

    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| "missing upstream choice".to_string())?;

    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| "missing upstream message object".to_string())?;
    let content_text = extract_openai_text_content(message.get("content").unwrap_or(&Value::Null))?;
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let stop_reason = if !tool_calls.is_empty() {
        "tool_use".to_string()
    } else {
        map_finish_reason(
            choice
                .get("finish_reason")
                .and_then(Value::as_str)
                .unwrap_or("stop"),
        )
        .to_string()
    };

    let mut content_blocks = Vec::new();
    if !content_text.is_empty() {
        content_blocks.push(json!({
            "type": "text",
            "text": content_text,
        }));
    }
    for (index, tool_call) in tool_calls.iter().enumerate() {
        let tool_use_id = tool_call
            .get("id")
            .and_then(Value::as_str)
            .map(|v| v.to_string())
            .unwrap_or_else(|| format!("toolu_{index}"));
        let function_name = tool_call
            .get("function")
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| "missing tool call function name".to_string())?;
        let function_name = restore_openai_tool_name(function_name, tool_name_restore_map);
        let arguments_raw = tool_call
            .get("function")
            .and_then(|item| item.get("arguments"))
            .and_then(Value::as_str)
            .unwrap_or("{}");
        let input = parse_tool_arguments_as_object(arguments_raw);
        content_blocks.push(json!({
            "type": "tool_use",
            "id": tool_use_id,
            "name": function_name,
            "input": input,
        }));
    }
    if content_blocks.is_empty() {
        content_blocks.push(json!({
            "type": "text",
            "text": "",
        }));
    }

    let usage = build_anthropic_usage(value.get("usage").and_then(Value::as_object));

    Ok(json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content_blocks,
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": usage,
    }))
}

/// 函数 `build_anthropic_message_from_responses`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
/// - tool_name_restore_map: 参数 tool_name_restore_map
///
/// # 返回
/// 返回函数执行结果
fn build_anthropic_message_from_responses(
    value: &Value,
    tool_name_restore_map: Option<&ToolNameRestoreMap>,
) -> Result<Value, String> {
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("msg_codexmanager");

    let mut content_blocks = Vec::new();
    let mut has_tool_use = false;
    let mut saw_message_text = false;

    if let Some(output_items) = value.get("output").and_then(Value::as_array) {
        for output_item in output_items {
            let Some(item_obj) = output_item.as_object() else {
                continue;
            };
            let item_type = item_obj
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match item_type {
                "message" => {
                    let content = item_obj.get("content").and_then(Value::as_array);
                    if let Some(content) = content {
                        for block in content {
                            let Some(block_obj) = block.as_object() else {
                                continue;
                            };
                            let block_type = block_obj
                                .get("type")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            if block_type == "output_text" || block_type == "text" {
                                if let Some(text) = block_obj.get("text").and_then(Value::as_str) {
                                    if push_anthropic_text_block(&mut content_blocks, text) {
                                        saw_message_text = true;
                                    }
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    let tool_use_id = item_obj
                        .get("call_id")
                        .or_else(|| item_obj.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("toolu_{}", content_blocks.len()));
                    let Some(function_name) = item_obj
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                    else {
                        continue;
                    };
                    let function_name =
                        restore_openai_tool_name(function_name, tool_name_restore_map);
                    let input = extract_function_call_input_object(item_obj);
                    content_blocks.push(json!({
                        "type": "tool_use",
                        "id": tool_use_id,
                        "name": function_name,
                        "input": input,
                    }));
                    has_tool_use = true;
                }
                "custom_tool_call" => {
                    let tool_use_id = item_obj
                        .get("call_id")
                        .or_else(|| item_obj.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("toolu_{}", content_blocks.len()));
                    let Some(function_name) = item_obj
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                    else {
                        continue;
                    };
                    let function_name =
                        restore_openai_tool_name(function_name, tool_name_restore_map);
                    let input = extract_function_call_input_object(item_obj);
                    content_blocks.push(json!({
                        "type": "tool_use",
                        "id": tool_use_id,
                        "name": function_name,
                        "input": input,
                    }));
                    has_tool_use = true;
                }
                "function_call_output" | "custom_tool_call_output" => {
                    if let Some(text) = extract_response_item_output_text(item_obj) {
                        if push_anthropic_text_block(&mut content_blocks, text.as_str()) {
                            saw_message_text = true;
                        }
                    }
                }
                "reasoning" => {
                    if let Some(block) = map_responses_reasoning_item_to_anthropic(item_obj) {
                        content_blocks.push(block);
                    }
                }
                _ => {
                    if let Some(summary) = summarize_special_response_item_text(item_obj) {
                        if push_anthropic_text_block(&mut content_blocks, summary.as_str()) {
                            saw_message_text = true;
                        }
                    }
                }
            }
        }
    }

    if !saw_message_text {
        if let Some(output_text) = value.get("output_text").and_then(Value::as_str) {
            push_anthropic_text_block(&mut content_blocks, output_text);
        }
    }

    if content_blocks.is_empty() {
        content_blocks.push(json!({
            "type": "text",
            "text": "",
        }));
    }

    let usage = build_anthropic_usage(value.get("usage").and_then(Value::as_object));

    let stop_reason = if has_tool_use {
        "tool_use".to_string()
    } else {
        "end_turn".to_string()
    };

    Ok(json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content_blocks,
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": usage,
    }))
}

/// 函数 `build_anthropic_usage`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - usage: 参数 usage
///
/// # 返回
/// 返回函数执行结果
fn build_anthropic_usage(usage: Option<&Map<String, Value>>) -> Value {
    let mut out = Map::new();
    out.insert(
        "input_tokens".to_string(),
        Value::from(extract_usage_i64(usage, &["input_tokens", "prompt_tokens"]).unwrap_or(0)),
    );
    out.insert(
        "output_tokens".to_string(),
        Value::from(extract_usage_i64(usage, &["output_tokens", "completion_tokens"]).unwrap_or(0)),
    );

    if let Some(cache_creation_input_tokens) = extract_usage_i64(
        usage,
        &[
            "cache_creation_input_tokens",
            "input_tokens_details.cache_creation_tokens",
        ],
    ) {
        out.insert(
            "cache_creation_input_tokens".to_string(),
            Value::from(cache_creation_input_tokens),
        );
    }
    if let Some(cache_read_input_tokens) = extract_usage_i64(
        usage,
        &[
            "cache_read_input_tokens",
            "input_tokens_details.cached_tokens",
            "prompt_tokens_details.cached_tokens",
        ],
    ) {
        out.insert(
            "cache_read_input_tokens".to_string(),
            Value::from(cache_read_input_tokens),
        );
    }

    Value::Object(out)
}

/// 函数 `extract_usage_i64`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - usage: 参数 usage
/// - paths: 参数 paths
///
/// # 返回
/// 返回函数执行结果
fn extract_usage_i64(usage: Option<&Map<String, Value>>, paths: &[&str]) -> Option<i64> {
    let usage = usage?;
    for path in paths {
        let mut cursor = None;
        for (index, segment) in path.split('.').enumerate() {
            cursor = if index == 0 {
                usage.get(segment)
            } else {
                cursor
                    .and_then(Value::as_object)
                    .and_then(|map| map.get(segment))
            };
        }
        if let Some(value) = cursor.and_then(Value::as_i64) {
            return Some(value);
        }
    }
    None
}

/// 函数 `push_anthropic_text_block`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - content_blocks: 参数 content_blocks
/// - text: 参数 text
///
/// # 返回
/// 返回函数执行结果
fn push_anthropic_text_block(content_blocks: &mut Vec<Value>, text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    if content_blocks
        .last()
        .and_then(Value::as_object)
        .is_some_and(|last| {
            last.get("type").and_then(Value::as_str) == Some("text")
                && last.get("text").and_then(Value::as_str) == Some(trimmed)
        })
    {
        return false;
    }

    content_blocks.push(json!({
        "type": "text",
        "text": trimmed,
    }));
    true
}

/// 函数 `summarize_special_response_item_text`
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
pub(super) fn summarize_special_response_item_text(
    item_obj: &serde_json::Map<String, Value>,
) -> Option<String> {
    let item_type = item_obj
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match item_type {
        "web_search_call" => summarize_web_search_call(item_obj),
        "image_generation_call" => summarize_image_generation_call(item_obj),
        "local_shell_call" => summarize_local_shell_call(item_obj),
        _ => None,
    }
}

/// 函数 `extract_response_item_output_text`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - item_obj: 参数 item_obj
///
/// # 返回
/// 返回函数执行结果
fn extract_response_item_output_text(item_obj: &serde_json::Map<String, Value>) -> Option<String> {
    /// 函数 `collect_output`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - value: 参数 value
    /// - out: 参数 out
    ///
    /// # 返回
    /// 无
    fn collect_output(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect_output(item, out);
                }
            }
            Value::Object(map) => {
                if let Some(text) = map.get("text").and_then(Value::as_str) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        out.push(trimmed.to_string());
                    }
                }
                if let Some(output) = map.get("output") {
                    collect_output(output, out);
                }
                if let Some(content) = map.get("content") {
                    collect_output(content, out);
                }
            }
            _ => {}
        }
    }

    let mut parts = Vec::new();
    if let Some(output) = item_obj.get("output") {
        collect_output(output, &mut parts);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// 函数 `summarize_web_search_call`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - item_obj: 参数 item_obj
///
/// # 返回
/// 返回函数执行结果
fn summarize_web_search_call(item_obj: &serde_json::Map<String, Value>) -> Option<String> {
    let status = item_obj
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let action = item_obj.get("action").and_then(Value::as_object);
    let summary = match action
        .and_then(|action| action.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "search" => {
            let query = action
                .and_then(|action| action.get("query"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let queries = action
                .and_then(|action| action.get("queries"))
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|items| !items.is_empty());
            if let Some(query) = query {
                format!("[web_search_call] status={status} query={query}")
            } else if let Some(queries) = queries {
                format!(
                    "[web_search_call] status={status} queries={}",
                    queries.join(" | ")
                )
            } else {
                format!("[web_search_call] status={status}")
            }
        }
        "open_page" => {
            let url = action
                .and_then(|action| action.get("url"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            url.map_or_else(
                || format!("[web_search_call] status={status} action=open_page"),
                |url| format!("[web_search_call] status={status} open_page={url}"),
            )
        }
        "find_in_page" => {
            let url = action
                .and_then(|action| action.get("url"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("-");
            let pattern = action
                .and_then(|action| action.get("pattern"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("-");
            format!("[web_search_call] status={status} find_in_page url={url} pattern={pattern}")
        }
        _ => format!("[web_search_call] status={status}"),
    };
    Some(summary)
}

/// 函数 `summarize_image_generation_call`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - item_obj: 参数 item_obj
///
/// # 返回
/// 返回函数执行结果
fn summarize_image_generation_call(item_obj: &serde_json::Map<String, Value>) -> Option<String> {
    let status = item_obj
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let revised_prompt = item_obj
        .get("revised_prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let result_hint = item_obj
        .get("result")
        .and_then(Value::as_str)
        .map(str::len)
        .filter(|len| *len > 0);
    let mut parts = vec![format!("[image_generation_call] status={status}")];
    if let Some(prompt) = revised_prompt {
        parts.push(format!("prompt={prompt}"));
    }
    if let Some(len) = result_hint {
        parts.push(format!("result_bytes={len}"));
    }
    Some(parts.join(" "))
}

/// 函数 `summarize_local_shell_call`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - item_obj: 参数 item_obj
///
/// # 返回
/// 返回函数执行结果
fn summarize_local_shell_call(item_obj: &serde_json::Map<String, Value>) -> Option<String> {
    let status = item_obj
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let action = item_obj.get("action").and_then(Value::as_object);
    let command = action
        .and_then(|action| action.get("command"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty());
    let working_directory = action
        .and_then(|action| action.get("working_directory"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut parts = vec![format!("[local_shell_call] status={status}")];
    if let Some(command) = command {
        parts.push(format!("command={}", command.join(" ")));
    }
    if let Some(working_directory) = working_directory {
        parts.push(format!("cwd={working_directory}"));
    }
    Some(parts.join(" "))
}

/// 函数 `map_responses_reasoning_item_to_anthropic`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - item_obj: 参数 item_obj
///
/// # 返回
/// 返回函数执行结果
fn map_responses_reasoning_item_to_anthropic(
    item_obj: &serde_json::Map<String, Value>,
) -> Option<Value> {
    let thinking = extract_responses_reasoning_text(item_obj);
    let signature = item_obj
        .get("encrypted_content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if thinking.is_empty() && signature.is_none() {
        return None;
    }

    let mut block = serde_json::Map::new();
    block.insert("type".to_string(), Value::String("thinking".to_string()));
    block.insert("thinking".to_string(), Value::String(thinking));
    if let Some(signature) = signature {
        block.insert("signature".to_string(), Value::String(signature));
    }
    Some(Value::Object(block))
}

/// 函数 `extract_responses_reasoning_text`
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
pub(super) fn extract_responses_reasoning_text(
    item_obj: &serde_json::Map<String, Value>,
) -> String {
    let content = collect_reasoning_text(item_obj.get("content"), true);
    if !content.is_empty() {
        return content;
    }
    collect_reasoning_text(item_obj.get("summary"), false)
}

/// 函数 `collect_reasoning_text`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
/// - content_mode: 参数 content_mode
///
/// # 返回
/// 返回函数执行结果
fn collect_reasoning_text(value: Option<&Value>, content_mode: bool) -> String {
    let Some(items) = value.and_then(Value::as_array) else {
        return String::new();
    };

    let mut parts = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let item_type = obj.get("type").and_then(Value::as_str).unwrap_or_default();
        let type_matches = if content_mode {
            matches!(item_type, "reasoning_text" | "text")
        } else {
            matches!(item_type, "summary_text" | "text")
        };
        if !type_matches {
            continue;
        }
        let Some(text) = obj
            .get("text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        parts.push(text.to_string());
    }

    if content_mode {
        parts.join("")
    } else {
        parts.join("\n\n")
    }
}

/// 函数 `extract_openai_text_content`
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
fn extract_openai_text_content(value: &Value) -> Result<String, String> {
    if value.is_null() {
        return Ok(String::new());
    }
    if let Some(text) = value.as_str() {
        return Ok(text.to_string());
    }
    let Some(items) = value.as_array() else {
        return Err("unsupported upstream content format".to_string());
    };
    let mut parts = Vec::new();
    for item in items {
        let Some(item_obj) = item.as_object() else {
            continue;
        };
        if item_obj
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "text")
        {
            if let Some(text) = item_obj.get("text").and_then(Value::as_str) {
                parts.push(text.to_string());
            }
        }
    }
    Ok(parts.join(""))
}

/// 函数 `parse_tool_arguments_as_object`
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
pub(super) fn parse_tool_arguments_as_object(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return json!({});
    }
    let parsed = serde_json::from_str::<Value>(trimmed).ok();
    match parsed {
        Some(Value::Object(obj)) => Value::Object(obj),
        Some(Value::String(inner)) => {
            let nested = serde_json::from_str::<Value>(inner.trim()).ok();
            if let Some(Value::Object(obj)) = nested {
                Value::Object(obj)
            } else {
                json!({ "value": inner })
            }
        }
        Some(other) => json!({ "value": other }),
        None => json!({}),
    }
}

/// 函数 `extract_function_call_input_object`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - item_obj: 参数 item_obj
///
/// # 返回
/// 返回函数执行结果
fn extract_function_call_input_object(item_obj: &serde_json::Map<String, Value>) -> Value {
    let Some(arguments_raw) = extract_function_call_arguments_raw(item_obj) else {
        return json!({});
    };
    parse_tool_arguments_as_object(&arguments_raw)
}

/// 函数 `extract_function_call_arguments_raw`
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
pub(super) fn extract_function_call_arguments_raw(
    item_obj: &serde_json::Map<String, Value>,
) -> Option<String> {
    const ARGUMENT_KEYS: [&str; 5] = [
        "arguments",
        "input",
        "arguments_json",
        "parsed_arguments",
        "args",
    ];
    for key in ARGUMENT_KEYS {
        let Some(value) = item_obj.get(key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        if let Some(text) = value.as_str() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
            continue;
        }
        if let Ok(serialized) = serde_json::to_string(value) {
            let trimmed = serialized.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// 函数 `map_finish_reason`
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
pub(super) fn map_finish_reason(reason: &str) -> &'static str {
    match reason {
        "tool_calls" => "tool_use",
        "length" => "max_tokens",
        "stop" => "end_turn",
        _ => "end_turn",
    }
}
