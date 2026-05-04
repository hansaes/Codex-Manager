use serde_json::{json, Value};

pub(crate) fn convert_openai_responses_request_to_anthropic_messages(
    body: &[u8],
) -> Result<(Vec<u8>, bool, super::ToolNameRestoreMap), String> {
    let payload: Value = serde_json::from_slice(body)
        .map_err(|_| "invalid responses request json".to_string())?;
    let Some(obj) = payload.as_object() else {
        return Err("responses request body must be an object".to_string());
    };

    let tool_name_map = collect_responses_tool_names(obj);
    let tool_name_restore_map = super::build_shortened_tool_name_maps(tool_name_map).1;
    let stream = obj.get("stream").and_then(Value::as_bool).unwrap_or(false);

    let mut messages = Vec::<Value>::new();
    if let Some(input) = obj.get("input") {
        match input {
            Value::String(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    messages.push(json!({
                        "role": "user",
                        "content": trimmed,
                    }));
                }
            }
            Value::Array(items) => {
                for item in items {
                    append_responses_input_item_as_anthropic_message(
                        item,
                        &mut messages,
                        Some(&tool_name_restore_map),
                    );
                }
            }
            Value::Object(_) => {
                append_responses_input_item_as_anthropic_message(
                    input,
                    &mut messages,
                    Some(&tool_name_restore_map),
                );
            }
            _ => {}
        }
    }

    let mut out = serde_json::Map::new();
    if let Some(model) = obj.get("model") {
        out.insert("model".to_string(), model.clone());
    }
    if let Some(instructions) = obj
        .get("instructions")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        out.insert("system".to_string(), Value::String(instructions.to_string()));
    }
    out.insert("messages".to_string(), Value::Array(messages));
    out.insert("stream".to_string(), Value::Bool(stream));

    if let Some(max_output_tokens) = obj.get("max_output_tokens") {
        out.insert("max_tokens".to_string(), max_output_tokens.clone());
    }
    if let Some(metadata) = obj.get("metadata") {
        out.insert("metadata".to_string(), metadata.clone());
    }
    if let Some(stop) = obj.get("stop") {
        let mapped = match stop {
            Value::String(_) => Value::Array(vec![stop.clone()]),
            Value::Array(_) => stop.clone(),
            _ => Value::Array(vec![]),
        };
        if mapped.as_array().is_some_and(|items| !items.is_empty()) {
            out.insert("stop_sequences".to_string(), mapped);
        }
    }
    if let Some(temperature) = obj.get("temperature") {
        out.insert("temperature".to_string(), temperature.clone());
    }
    if let Some(top_p) = obj.get("top_p") {
        out.insert("top_p".to_string(), top_p.clone());
    }

    let reasoning_effort = obj
        .get("reasoning_effort")
        .and_then(Value::as_str)
        .and_then(crate::reasoning_effort::normalize_reasoning_effort)
        .or_else(|| {
            obj.get("reasoning")
                .and_then(|reasoning| reasoning.get("effort"))
                .and_then(Value::as_str)
                .and_then(crate::reasoning_effort::normalize_reasoning_effort)
        });
    if let Some(reasoning_effort) = reasoning_effort {
        out.insert(
            "output_config".to_string(),
            json!({
                "effort": reasoning_effort,
            }),
        );
    }

    let thinking_enabled = obj
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("summary"))
        .and_then(Value::as_str)
        .map(|summary| !summary.trim().eq_ignore_ascii_case("none"))
        .or_else(|| {
            obj.get("reasoning")
                .and_then(|reasoning| reasoning.get("effort"))
                .and_then(Value::as_str)
                .map(|_| true)
        })
        .unwrap_or(false);
    if thinking_enabled {
        out.insert(
            "thinking".to_string(),
            json!({
                "type": "enabled",
                "budget_tokens": 1024
            }),
        );
    }

    let mapped_tools = map_responses_tools_to_anthropic(obj, &tool_name_restore_map);
    if !mapped_tools.is_empty() {
        out.insert("tools".to_string(), Value::Array(mapped_tools));
        if !obj.contains_key("tool_choice") {
            out.insert("tool_choice".to_string(), json!({ "type": "auto" }));
        }
    }
    if let Some(tool_choice) = obj
        .get("tool_choice")
        .and_then(|value| map_responses_tool_choice_to_anthropic(value, &tool_name_restore_map))
    {
        out.insert("tool_choice".to_string(), tool_choice);
    }

    serde_json::to_vec(&Value::Object(out))
        .map(|bytes| (bytes, stream, tool_name_restore_map))
        .map_err(|err| format!("convert responses request failed: {err}"))
}

/// 函数 `convert_anthropic_messages_request`
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
pub(crate) fn convert_anthropic_messages_request(
    body: &[u8],
) -> Result<(Vec<u8>, bool, super::ToolNameRestoreMap), String> {
    let payload: Value =
        serde_json::from_slice(body).map_err(|_| "invalid claude request json".to_string())?;
    let Some(obj) = payload.as_object() else {
        return Err("claude request body must be an object".to_string());
    };

    let mut messages = Vec::new();

    if let Some(system) = obj.get("system") {
        let system_text = extract_text_content(system)?;
        if !system_text.trim().is_empty() {
            messages.push(json!({
                "role": "system",
                "content": system_text,
            }));
        }
    }

    let source_messages = obj
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "claude messages field is required".to_string())?;
    for message in source_messages {
        let Some(message_obj) = message.as_object() else {
            return Err("invalid claude message item".to_string());
        };
        let role = message_obj
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| "claude message role is required".to_string())?;
        let content = message_obj
            .get("content")
            .ok_or_else(|| "claude message content is required".to_string())?;
        match role {
            "assistant" => append_assistant_messages(&mut messages, content)?,
            "user" => append_user_messages(&mut messages, content)?,
            "tool" => append_tool_role_message(&mut messages, message_obj, content)?,
            other => return Err(format!("unsupported claude message role: {other}")),
        }
    }

    let (tool_name_map, tool_name_restore_map) =
        super::build_shortened_tool_name_maps(collect_anthropic_tool_names(obj, source_messages));
    let (instructions, input_items) =
        super::convert_chat_messages_to_responses_input(&messages, &tool_name_map)?;
    let mut out = serde_json::Map::new();
    let resolved_model = resolve_anthropic_upstream_model(obj)?;
    out.insert("model".to_string(), Value::String(resolved_model));
    let resolved_instructions = instructions
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(super::DEFAULT_ANTHROPIC_INSTRUCTIONS);
    out.insert(
        "instructions".to_string(),
        Value::String(resolved_instructions.to_string()),
    );
    out.insert(
        "text".to_string(),
        json!({
            "format": {
                "type": "text",
            }
        }),
    );
    let resolved_reasoning = resolve_anthropic_reasoning_effort(obj).to_string();
    let mut reasoning = serde_json::Map::new();
    reasoning.insert(
        "effort".to_string(),
        Value::String(resolved_reasoning.clone()),
    );
    if let Some(summary) = resolve_anthropic_reasoning_summary(obj) {
        reasoning.insert("summary".to_string(), Value::String(summary.to_string()));
    }
    out.insert("reasoning".to_string(), Value::Object(reasoning));
    out.insert("input".to_string(), Value::Array(input_items));
    if let Some(encrypted_content) = extract_latest_anthropic_thinking_signature(source_messages) {
        out.insert(
            "encrypted_content".to_string(),
            Value::String(encrypted_content),
        );
    }

    if let Some(prompt_cache_key) = super::resolve_prompt_cache_key(obj, out.get("model")) {
        out.insert(
            "prompt_cache_key".to_string(),
            Value::String(prompt_cache_key),
        );
    }
    if let Some(tools) = obj.get("tools").and_then(Value::as_array) {
        let mapped_tools = tools
            .iter()
            .filter_map(|tool| super::map_anthropic_tool_definition(tool, &tool_name_map))
            .collect::<Vec<_>>();
        if !mapped_tools.is_empty() {
            out.insert("tools".to_string(), Value::Array(mapped_tools));
            if !obj.contains_key("tool_choice") {
                out.insert("tool_choice".to_string(), Value::String("auto".to_string()));
            }
        }
    }
    if let Some(tool_choice) = obj.get("tool_choice") {
        if !tool_choice.is_null() {
            if let Some(mapped_tool_choice) =
                super::map_anthropic_tool_choice(tool_choice, &tool_name_map)
            {
                out.insert("tool_choice".to_string(), mapped_tool_choice);
            }
        }
    }
    if !out.contains_key("tool_choice") {
        out.insert("tool_choice".to_string(), Value::String("auto".to_string()));
    }
    let request_stream = obj.get("stream").and_then(Value::as_bool).unwrap_or(true);
    out.insert("stream".to_string(), Value::Bool(true));
    out.insert(
        "parallel_tool_calls".to_string(),
        Value::Bool(super::resolve_anthropic_parallel_tool_calls(obj)),
    );
    out.insert("store".to_string(), Value::Bool(false));
    out.insert(
        "include".to_string(),
        Value::Array(vec![Value::String(
            "reasoning.encrypted_content".to_string(),
        )]),
    );

    serde_json::to_vec(&Value::Object(out))
        .map(|bytes| (bytes, request_stream, tool_name_restore_map))
        .map_err(|err| format!("convert claude request failed: {err}"))
}

/// 函数 `collect_anthropic_tool_names`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - obj: 参数 obj
/// - source_messages: 参数 source_messages
///
/// # 返回
/// 返回函数执行结果
fn collect_anthropic_tool_names(
    obj: &serde_json::Map<String, Value>,
    source_messages: &[Value],
) -> Vec<String> {
    let mut names = Vec::new();

    if let Some(tools) = obj.get("tools").and_then(Value::as_array) {
        for tool in tools {
            let Some(tool_obj) = tool.as_object() else {
                continue;
            };
            let Some(name) = tool_obj
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| tool_obj.get("type").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            names.push(name.to_string());
        }
    }

    if let Some(name) = obj
        .get("tool_choice")
        .and_then(Value::as_object)
        .and_then(|tool_choice| {
            if tool_choice.get("type").and_then(Value::as_str) != Some("tool") {
                return None;
            }
            tool_choice
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
    {
        names.push(name.to_string());
    }

    for message in source_messages {
        let Some(message_obj) = message.as_object() else {
            continue;
        };
        let Some(content) = message_obj.get("content") else {
            continue;
        };
        let items = if let Some(array) = content.as_array() {
            array
        } else {
            continue;
        };
        for item in items {
            let Some(item_obj) = item.as_object() else {
                continue;
            };
            if item_obj.get("type").and_then(Value::as_str) != Some("tool_use") {
                continue;
            }
            let Some(name) = item_obj
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            names.push(name.to_string());
        }
    }

    names
}

/// 函数 `resolve_anthropic_upstream_model`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - source: 参数 source
///
/// # 返回
/// 返回函数执行结果
fn resolve_anthropic_upstream_model(
    source: &serde_json::Map<String, Value>,
) -> Result<String, String> {
    let requested_model = source
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    requested_model
        .map(str::to_string)
        .ok_or_else(|| "claude model is required".to_string())
}

/// 函数 `resolve_anthropic_reasoning_effort`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - source: 参数 source
///
/// # 返回
/// 返回函数执行结果
fn resolve_anthropic_reasoning_effort(source: &serde_json::Map<String, Value>) -> &'static str {
    source
        .get("reasoning")
        .and_then(Value::as_object)
        .and_then(|value| value.get("effort"))
        .and_then(Value::as_str)
        .or_else(|| {
            source
                .get("output_config")
                .and_then(Value::as_object)
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str)
        })
        .or_else(|| source.get("effort").and_then(Value::as_str))
        .and_then(crate::reasoning_effort::normalize_reasoning_effort)
        .unwrap_or(super::DEFAULT_ANTHROPIC_REASONING)
}

/// 函数 `resolve_anthropic_reasoning_summary`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - source: 参数 source
///
/// # 返回
/// 返回函数执行结果
fn resolve_anthropic_reasoning_summary(
    source: &serde_json::Map<String, Value>,
) -> Option<&'static str> {
    match source.get("thinking") {
        Some(Value::Bool(true)) => Some("detailed"),
        Some(Value::Bool(false)) => Some("none"),
        Some(Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
            "enabled" | "on" | "true" => Some("detailed"),
            "disabled" | "off" | "false" => Some("none"),
            _ => None,
        },
        Some(Value::Object(obj)) => {
            let thinking_type = obj
                .get("type")
                .and_then(Value::as_str)
                .map(|value| value.trim().to_ascii_lowercase());
            match thinking_type.as_deref() {
                Some("disabled") => Some("none"),
                Some("enabled") => Some("detailed"),
                _ if obj
                    .get("budget_tokens")
                    .and_then(Value::as_i64)
                    .is_some_and(|value| value > 0) =>
                {
                    Some("detailed")
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// 函数 `extract_latest_anthropic_thinking_signature`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - messages: 参数 messages
///
/// # 返回
/// 返回函数执行结果
fn extract_latest_anthropic_thinking_signature(messages: &[Value]) -> Option<String> {
    for message in messages.iter().rev() {
        let Some(message_obj) = message.as_object() else {
            continue;
        };
        let Some(content) = message_obj.get("content") else {
            continue;
        };
        let blocks = if let Some(array) = content.as_array() {
            array
        } else if content.is_object() {
            std::slice::from_ref(content)
        } else {
            continue;
        };
        for block in blocks.iter().rev() {
            let Some(block_obj) = block.as_object() else {
                continue;
            };
            let block_type = block_obj
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !matches!(block_type, "thinking" | "redacted_thinking") {
                continue;
            }
            let signature = block_obj
                .get("signature")
                .or_else(|| block_obj.get("encrypted_content"))
                .or_else(|| block_obj.get("data"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some(signature) = signature {
                return Some(signature.to_string());
            }
        }
    }
    None
}

/// 函数 `append_assistant_messages`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - messages: 参数 messages
/// - content: 参数 content
///
/// # 返回
/// 返回函数执行结果
fn append_assistant_messages(messages: &mut Vec<Value>, content: &Value) -> Result<(), String> {
    if let Some(text) = content.as_str() {
        messages.push(json!({
            "role": "assistant",
            "content": text,
        }));
        return Ok(());
    }

    let blocks = if let Some(array) = content.as_array() {
        array.to_vec()
    } else if content.is_object() {
        vec![content.clone()]
    } else {
        return Err("unsupported assistant content".to_string());
    };

    let mut content_parts = Vec::new();
    for block in blocks {
        let Some(block_obj) = block.as_object() else {
            return Err("invalid assistant content block".to_string());
        };
        let block_type = block_obj
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| "assistant content block missing type".to_string())?;
        match block_type {
            "text" => {
                if let Some(text) = block_obj.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        content_parts.push(json!({
                            "type": "text",
                            "text": text,
                        }));
                    }
                }
            }
            "tool_use" => {
                let id = block_obj
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("toolu_{}", content_parts.len()));
                let Some(name) = block_obj
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                else {
                    continue;
                };
                content_parts.push(json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": block_obj.get("input").cloned().unwrap_or_else(|| json!({})),
                }));
            }
            _ => continue,
        }
    }

    if content_parts.is_empty() {
        return Ok(());
    }
    messages.push(json!({
        "role": "assistant",
        "content": content_parts,
    }));
    Ok(())
}

/// 函数 `append_user_messages`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - messages: 参数 messages
/// - content: 参数 content
///
/// # 返回
/// 返回函数执行结果
fn append_user_messages(messages: &mut Vec<Value>, content: &Value) -> Result<(), String> {
    if let Some(text) = content.as_str() {
        if !text.trim().is_empty() {
            messages.push(json!({
                "role": "user",
                "content": text,
            }));
        }
        return Ok(());
    }

    let blocks = if let Some(array) = content.as_array() {
        array.to_vec()
    } else if content.is_object() {
        vec![content.clone()]
    } else {
        return Err("unsupported user content".to_string());
    };

    let mut pending_parts = Vec::new();
    for block in blocks {
        let Some(block_obj) = block.as_object() else {
            return Err("invalid user content block".to_string());
        };
        let block_type = block_obj
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| "user content block missing type".to_string())?;
        match block_type {
            "text" => {
                if let Some(text) = block_obj.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        pending_parts.push(json!({
                            "type": "input_text",
                            "text": text,
                        }));
                    }
                }
            }
            "image" => {
                if let Some(image_item) =
                    super::map_anthropic_image_block_to_responses_item(block_obj)
                {
                    pending_parts.push(image_item);
                }
            }
            "tool_result" => {
                flush_user_content_parts(messages, &mut pending_parts);
                let tool_use_id = block_obj
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .or_else(|| block_obj.get("id").and_then(Value::as_str))
                    .unwrap_or_default();
                if tool_use_id.is_empty() {
                    continue;
                }
                let mut tool_content = super::extract_tool_result_output(block_obj.get("content"))?;
                if block_obj
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    tool_content = super::prefix_tool_error_output(tool_content);
                }
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": tool_content,
                }));
            }
            _ => continue,
        }
    }
    flush_user_content_parts(messages, &mut pending_parts);
    Ok(())
}

/// 函数 `append_tool_role_message`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - messages: 参数 messages
/// - message_obj: 参数 message_obj
/// - content: 参数 content
///
/// # 返回
/// 返回函数执行结果
fn append_tool_role_message(
    messages: &mut Vec<Value>,
    message_obj: &serde_json::Map<String, Value>,
    content: &Value,
) -> Result<(), String> {
    let tool_call_id = message_obj
        .get("tool_call_id")
        .or_else(|| message_obj.get("tool_use_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "tool role message missing tool_call_id".to_string())?;
    let tool_content = super::extract_tool_result_output(Some(content))?;
    messages.push(json!({
        "role": "tool",
        "tool_call_id": tool_call_id,
        "content": tool_content,
    }));
    Ok(())
}

/// 函数 `flush_user_content_parts`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - messages: 参数 messages
/// - pending_parts: 参数 pending_parts
///
/// # 返回
/// 无
fn flush_user_content_parts(messages: &mut Vec<Value>, pending_parts: &mut Vec<Value>) {
    if pending_parts.is_empty() {
        return;
    }
    messages.push(json!({
        "role": "user",
        "content": pending_parts.clone(),
    }));
    pending_parts.clear();
}

/// 函数 `extract_text_content`
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
fn extract_text_content(value: &Value) -> Result<String, String> {
    if let Some(text) = value.as_str() {
        return Ok(text.to_string());
    }

    if let Some(block) = value.as_object() {
        return extract_text_from_block(block);
    }

    if let Some(array) = value.as_array() {
        let mut parts = Vec::new();
        for item in array {
            let Some(block) = item.as_object() else {
                return Err("invalid claude content block".to_string());
            };
            parts.push(extract_text_from_block(block)?);
        }
        return Ok(parts.join(""));
    }

    Err("unsupported claude content".to_string())
}

/// 函数 `extract_text_from_block`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - block: 参数 block
///
/// # 返回
/// 返回函数执行结果
fn extract_text_from_block(block: &serde_json::Map<String, Value>) -> Result<String, String> {
    let block_type = block
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "claude content block missing type".to_string())?;
    if block_type != "text" {
        return Err(format!(
            "unsupported claude content block type: {block_type}"
        ));
    }
    block
        .get("text")
        .and_then(Value::as_str)
        .map(|v| v.to_string())
        .ok_or_else(|| "claude text block missing text".to_string())
}

fn collect_responses_tool_names(obj: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut names = Vec::new();

    if let Some(tools) = obj.get("tools").and_then(Value::as_array) {
        for tool in tools {
            let Some(tool_obj) = tool.as_object() else {
                continue;
            };
            let Some(name) = tool_obj
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| {
                    tool_obj
                        .get("function")
                        .and_then(|function| function.get("name"))
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            names.push(name.to_string());
        }
    }

    if let Some(name) = obj
        .get("tool_choice")
        .and_then(Value::as_object)
        .and_then(|tool_choice| {
            tool_choice
                .get("name")
                .or_else(|| {
                    tool_choice
                        .get("function")
                        .and_then(|function| function.get("name"))
                })
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
    {
        names.push(name.to_string());
    }

    if let Some(input) = obj.get("input") {
        collect_responses_tool_names_from_input(input, &mut names);
    }

    names
}

fn collect_responses_tool_names_from_input(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_responses_tool_names_from_input(item, out);
            }
        }
        Value::Object(obj) => {
            let item_type = obj.get("type").and_then(Value::as_str).unwrap_or_default();
            if matches!(item_type, "function_call" | "custom_tool_call") {
                if let Some(name) = obj
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    out.push(name.to_string());
                }
            }
            if item_type == "message" {
                if let Some(content) = obj.get("content").and_then(Value::as_array) {
                    for part in content {
                        collect_responses_tool_names_from_input(part, out);
                    }
                }
            }
        }
        _ => {}
    }
}

fn append_responses_input_item_as_anthropic_message(
    item: &Value,
    out: &mut Vec<Value>,
    tool_name_restore_map: Option<&super::ToolNameRestoreMap>,
) {
    let Some(item_obj) = item.as_object() else {
        return;
    };
    let item_type = item_obj.get("type").and_then(Value::as_str).unwrap_or_default();
    match item_type {
        "function_call" | "custom_tool_call" => {
            let Some(name) = item_obj
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return;
            };
            let call_id = item_obj
                .get("call_id")
                .or_else(|| item_obj.get("id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("toolu_0");
            let restored_name = restore_tool_name(name, tool_name_restore_map);
            let input = extract_function_input(item_obj);
            out.push(json!({
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": call_id,
                    "name": restored_name,
                    "input": input,
                }]
            }));
        }
        "function_call_output" | "custom_tool_call_output" => {
            let call_id = item_obj
                .get("call_id")
                .or_else(|| item_obj.get("id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or_default();
            if call_id.is_empty() {
                return;
            }
            let content = map_responses_tool_output_to_anthropic_content(item_obj.get("output"));
            out.push(json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": call_id,
                    "content": content,
                }]
            }));
        }
        "message" => {
            let role = item_obj
                .get("role")
                .and_then(Value::as_str)
                .map(normalize_responses_role_to_anthropic)
                .unwrap_or("user");
            let content = map_responses_message_content_to_anthropic(item_obj.get("content"));
            if content.is_null() {
                return;
            }
            out.push(json!({
                "role": role,
                "content": content,
            }));
        }
        _ => {
            if let Some(text) = extract_responses_item_text(item_obj) {
                out.push(json!({
                    "role": "user",
                    "content": text,
                }));
            }
        }
    }
}

fn normalize_responses_role_to_anthropic(role: &str) -> &'static str {
    match role.trim().to_ascii_lowercase().as_str() {
        "assistant" => "assistant",
        _ => "user",
    }
}

fn map_responses_message_content_to_anthropic(content: Option<&Value>) -> Value {
    let Some(content) = content else {
        return Value::Null;
    };
    if let Some(text) = content.as_str() {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Value::Null;
        }
        return Value::String(trimmed.to_string());
    }
    let Some(items) = content.as_array() else {
        return Value::Null;
    };
    let mut blocks = Vec::new();
    for item in items {
        let Some(item_obj) = item.as_object() else {
            continue;
        };
        match item_obj.get("type").and_then(Value::as_str).unwrap_or_default() {
            "input_text" | "output_text" | "text" => {
                if let Some(text) = item_obj
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    blocks.push(json!({
                        "type": "text",
                        "text": text,
                    }));
                }
            }
            "input_image" => {
                if let Some(block) = map_responses_image_to_anthropic(item_obj) {
                    blocks.push(block);
                }
            }
            _ => {}
        }
    }
    if blocks.is_empty() {
        Value::Null
    } else {
        Value::Array(blocks)
    }
}

fn map_responses_image_to_anthropic(
    item_obj: &serde_json::Map<String, Value>,
) -> Option<Value> {
    let image_url = item_obj
        .get("image_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if let Some(data) = image_url.strip_prefix("data:") {
        let (meta, encoded) = data.split_once(',')?;
        let media_type = meta
            .split(';')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("image/png");
        return Some(json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": media_type,
                "data": encoded,
            }
        }));
    }
    Some(json!({
        "type": "image",
        "source": {
            "type": "url",
            "url": image_url,
        }
    }))
}

fn map_responses_tool_output_to_anthropic_content(output: Option<&Value>) -> Value {
    let Some(output) = output else {
        return Value::String(String::new());
    };
    if let Some(text) = output.as_str() {
        return Value::String(text.to_string());
    }
    if let Some(items) = output.as_array() {
        let mut blocks = Vec::new();
        for item in items {
            let Some(item_obj) = item.as_object() else {
                continue;
            };
            match item_obj.get("type").and_then(Value::as_str).unwrap_or_default() {
                "input_text" | "text" | "output_text" => {
                    if let Some(text) = item_obj
                        .get("text")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        blocks.push(json!({
                            "type": "text",
                            "text": text,
                        }));
                    }
                }
                "input_image" => {
                    if let Some(block) = map_responses_image_to_anthropic(item_obj) {
                        blocks.push(block);
                    }
                }
                _ => {}
            }
        }
        return if blocks.is_empty() {
            Value::String(String::new())
        } else {
            Value::Array(blocks)
        };
    }
    output.clone()
}

fn extract_function_input(item_obj: &serde_json::Map<String, Value>) -> Value {
    item_obj
        .get("arguments")
        .and_then(Value::as_str)
        .and_then(|arguments| serde_json::from_str(arguments).ok())
        .or_else(|| item_obj.get("input").cloned())
        .unwrap_or_else(|| json!({}))
}

fn restore_tool_name(
    name: &str,
    tool_name_restore_map: Option<&super::ToolNameRestoreMap>,
) -> String {
    tool_name_restore_map
        .and_then(|map| map.get(name))
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

fn map_responses_tools_to_anthropic(
    obj: &serde_json::Map<String, Value>,
    tool_name_restore_map: &super::ToolNameRestoreMap,
) -> Vec<Value> {
    let Some(tools) = obj.get("tools").and_then(Value::as_array) else {
        return Vec::new();
    };
    tools
        .iter()
        .filter_map(|tool| map_responses_tool_to_anthropic(tool, tool_name_restore_map))
        .collect()
}

fn map_responses_tool_to_anthropic(
    tool: &Value,
    tool_name_restore_map: &super::ToolNameRestoreMap,
) -> Option<Value> {
    let obj = tool.as_object()?;
    let tool_type = obj.get("type").and_then(Value::as_str).unwrap_or("function");
    if tool_type != "function" {
        return None;
    }
    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| {
            obj.get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let restored_name = restore_tool_name(name, Some(tool_name_restore_map));
    let description = obj
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let input_schema = obj
        .get("parameters")
        .or_else(|| obj.get("input_schema"))
        .cloned()
        .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
    Some(json!({
        "name": restored_name,
        "description": description,
        "input_schema": input_schema,
    }))
}

fn map_responses_tool_choice_to_anthropic(
    value: &Value,
    tool_name_restore_map: &super::ToolNameRestoreMap,
) -> Option<Value> {
    if let Some(text) = value.as_str() {
        return match text.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(json!({ "type": "auto" })),
            "required" => Some(json!({ "type": "any" })),
            "none" => Some(json!({ "type": "none" })),
            _ => None,
        };
    }
    let obj = value.as_object()?;
    let choice_type = obj.get("type").and_then(Value::as_str).unwrap_or_default();
    if choice_type != "function" {
        return None;
    }
    let name = obj
        .get("name")
        .or_else(|| {
            obj.get("function")
                .and_then(|function| function.get("name"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(json!({
        "type": "tool",
        "name": restore_tool_name(name, Some(tool_name_restore_map)),
    }))
}

fn extract_responses_item_text(item_obj: &serde_json::Map<String, Value>) -> Option<String> {
    if let Some(text) = item_obj
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_string());
    }
    item_obj
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| {
            let parts = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(""))
            }
        })
}
