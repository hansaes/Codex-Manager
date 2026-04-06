use rand::{distributions::Alphanumeric, Rng};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// 函数 `convert_gemini_generate_content_request`
///
/// 作者: Codex
///
/// 时间: 2026-04-05
///
/// # 参数
/// - path: Gemini 原生请求路径
/// - body: Gemini 原生请求体
///
/// # 返回
/// 返回转换后的 Responses 请求体、是否流式和工具名还原映射
pub(crate) fn convert_gemini_generate_content_request(
    path: &str,
    body: &[u8],
) -> Result<(Vec<u8>, bool, super::ToolNameRestoreMap), String> {
    let payload: Value =
        serde_json::from_slice(body).map_err(|_| "invalid gemini request json".to_string())?;
    let (source, body_model) = extract_gemini_request_source(payload)?;
    let obj = &source;

    let model = extract_model_from_path(path)
        .or(body_model)
        .ok_or_else(|| "gemini model is required".to_string())?;
    let request_stream = normalized_request_path(path).contains(":streamGenerateContent");
    let tool_names = collect_gemini_declared_tool_names(obj);
    let (tool_name_map, tool_name_restore_map) = build_gemini_cpa_tool_name_maps(tool_names);
    let mut input_items = convert_gemini_contents_to_cpa_responses_input(obj, &tool_name_map)?;
    if let Some(system_message) = build_gemini_system_instruction_message(obj) {
        input_items.insert(0, system_message);
    }

    let mut out = serde_json::Map::new();
    out.insert("model".to_string(), Value::String(model));
    out.insert("instructions".to_string(), Value::String(String::new()));
    out.insert("input".to_string(), Value::Array(input_items));
    out.insert("stream".to_string(), Value::Bool(true));
    out.insert("store".to_string(), Value::Bool(false));
    out.insert("parallel_tool_calls".to_string(), Value::Bool(true));

    if let Some(tools) = map_gemini_tools_to_cpa_responses(obj, &tool_name_map) {
        out.insert("tools".to_string(), Value::Array(tools));
        out.insert("tool_choice".to_string(), Value::String("auto".to_string()));
    }

    let effort = resolve_gemini_cpa_reasoning_effort(obj);
    out.insert(
        "reasoning".to_string(),
        json!({ "effort": effort, "summary": "auto" }),
    );
    out.insert(
        "include".to_string(),
        Value::Array(vec![Value::String(
            "reasoning.encrypted_content".to_string(),
        )]),
    );

    serde_json::to_vec(&Value::Object(out))
        .map(|bytes| (bytes, request_stream, tool_name_restore_map))
        .map_err(|err| format!("convert gemini request failed: {err}"))
}

fn extract_gemini_request_source(
    payload: Value,
) -> Result<(serde_json::Map<String, Value>, Option<String>), String> {
    let Some(root) = payload.as_object() else {
        return Err("gemini request body must be an object".to_string());
    };
    let body_model = root
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut source = root
        .get("request")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| root.clone());
    if source.contains_key("systemInstruction") && !source.contains_key("system_instruction") {
        if let Some(value) = source.remove("systemInstruction") {
            source.insert("system_instruction".to_string(), value);
        }
    }
    Ok((source, body_model))
}

fn build_gemini_system_instruction_message(
    source: &serde_json::Map<String, Value>,
) -> Option<Value> {
    let system = get_value_field(source, &["system_instruction", "systemInstruction"])?;
    let parts = system.get("parts").and_then(Value::as_array)?;
    let mut content_parts = Vec::new();
    for part in parts {
        let Some(text) = part
            .get("text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        content_parts.push(json!({ "type": "input_text", "text": text }));
    }
    if content_parts.is_empty() {
        return None;
    }
    Some(json!({
        "type": "message",
        "role": "developer",
        "content": content_parts,
    }))
}

fn convert_gemini_contents_to_cpa_responses_input(
    source: &serde_json::Map<String, Value>,
    tool_name_map: &BTreeMap<String, String>,
) -> Result<Vec<Value>, String> {
    let mut items = Vec::new();
    let mut pending_call_ids: VecDeque<String> = VecDeque::new();
    let contents = source
        .get("contents")
        .and_then(Value::as_array)
        .ok_or_else(|| "gemini contents field is required".to_string())?;
    for content in contents {
        let role = content
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let role = if role == "model" { "assistant" } else { role };
        let Some(parts) = content.get("parts").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if let Some(text) = part
                .get("text")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let part_type = if role == "assistant" {
                    "output_text"
                } else {
                    "input_text"
                };
                items.push(json!({
                    "type": "message",
                    "role": role,
                    "content": [{ "type": part_type, "text": text }],
                }));
                continue;
            }

            if let Some(function_call) = part.get("functionCall").and_then(Value::as_object) {
                let Some(name) = function_call
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let mapped = map_gemini_cpa_tool_name(name, tool_name_map);
                let call_id = generate_gemini_call_id();
                pending_call_ids.push_back(call_id.clone());
                let arguments = function_call
                    .get("args")
                    .and_then(|value| serde_json::to_string(value).ok())
                    .unwrap_or_else(|| "{}".to_string());
                items.push(json!({
                    "type": "function_call",
                    "name": mapped,
                    "arguments": arguments,
                    "call_id": call_id,
                }));
                continue;
            }

            if let Some(function_response) = part.get("functionResponse").and_then(Value::as_object)
            {
                let output = if let Some(result) = function_response
                    .get("response")
                    .and_then(|value| value.get("result"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    result.to_string()
                } else if let Some(response) = function_response.get("response") {
                    serde_json::to_string(response).unwrap_or_default()
                } else {
                    String::new()
                };
                let call_id = pending_call_ids
                    .pop_front()
                    .unwrap_or_else(generate_gemini_call_id);
                items.push(json!({
                    "type": "function_call_output",
                    "output": output,
                    "call_id": call_id,
                }));
            }
        }
    }
    Ok(items)
}

fn generate_gemini_call_id() -> String {
    let rand_text: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect();
    format!("call_{rand_text}")
}

fn collect_gemini_declared_tool_names(source: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut names = Vec::new();
    let Some(tools) = source.get("tools").and_then(Value::as_array) else {
        return names;
    };
    for tool in tools {
        let Some(tool_obj) = tool.as_object() else {
            continue;
        };
        let Some(function_declarations) =
            get_array_field(tool_obj, &["functionDeclarations", "function_declarations"])
        else {
            continue;
        };
        for declaration in function_declarations {
            let Some(name) = declaration
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

fn build_gemini_cpa_tool_name_maps(
    names: Vec<String>,
) -> (BTreeMap<String, String>, super::ToolNameRestoreMap) {
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    for name in names {
        let trimmed = name.trim();
        if trimmed.is_empty() || seen.contains(trimmed) {
            continue;
        }
        seen.insert(trimmed.to_string());
        ordered.push(trimmed.to_string());
    }

    let mut used = BTreeSet::new();
    let mut tool_name_map = BTreeMap::new();
    let mut restore_map = super::ToolNameRestoreMap::new();
    for original in ordered {
        let base = gemini_cpa_short_candidate(original.as_str());
        let unique = gemini_cpa_make_unique(&base, &mut used);
        if original != unique {
            restore_map.insert(unique.clone(), original.clone());
        }
        tool_name_map.insert(original, unique);
    }
    (tool_name_map, restore_map)
}

fn gemini_cpa_short_candidate(name: &str) -> String {
    const LIMIT: usize = 64;
    if name.len() <= LIMIT {
        return name.to_string();
    }
    if name.starts_with("mcp__") {
        if let Some(idx) = name.rfind("__") {
            if idx > 0 {
                let mut candidate = format!("mcp__{}", &name[idx + 2..]);
                if candidate.len() > LIMIT {
                    candidate.truncate(LIMIT);
                }
                return candidate;
            }
        }
    }
    name.chars().take(LIMIT).collect()
}

fn gemini_cpa_make_unique(base: &str, used: &mut BTreeSet<String>) -> String {
    const LIMIT: usize = 64;
    if !used.contains(base) {
        used.insert(base.to_string());
        return base.to_string();
    }
    for idx in 1usize.. {
        let suffix = format!("_{idx}");
        let allowed = LIMIT.saturating_sub(suffix.len());
        let mut prefix = base.to_string();
        if prefix.len() > allowed {
            prefix.truncate(allowed);
        }
        let candidate = format!("{prefix}{suffix}");
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
    }
    base.to_string()
}

fn map_gemini_cpa_tool_name(name: &str, tool_name_map: &BTreeMap<String, String>) -> String {
    tool_name_map
        .get(name)
        .cloned()
        .unwrap_or_else(|| gemini_cpa_short_candidate(name))
}

fn map_gemini_tools_to_cpa_responses(
    source: &serde_json::Map<String, Value>,
    tool_name_map: &BTreeMap<String, String>,
) -> Option<Vec<Value>> {
    let Some(tools) = source.get("tools").and_then(Value::as_array) else {
        return None;
    };
    let mut out = Vec::new();
    for tool in tools {
        let Some(tool_obj) = tool.as_object() else {
            continue;
        };
        let Some(function_declarations) =
            get_array_field(tool_obj, &["functionDeclarations", "function_declarations"])
        else {
            continue;
        };
        for declaration in function_declarations {
            let Some(name) = declaration
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let mapped_name = map_gemini_cpa_tool_name(name, tool_name_map);
            let mut mapped = serde_json::Map::new();
            mapped.insert("type".to_string(), Value::String("function".to_string()));
            mapped.insert("name".to_string(), Value::String(mapped_name));
            if let Some(description) = declaration.get("description") {
                mapped.insert("description".to_string(), description.clone());
            }
            if let Some(parameters) = get_value_field(
                declaration.as_object().expect("declaration object"),
                &[
                    "parameters",
                    "parametersJsonSchema",
                    "parameters_json_schema",
                ],
            ) {
                let cleaned = clean_gemini_tool_schema(parameters);
                mapped.insert("parameters".to_string(), cleaned);
            }
            mapped.insert("strict".to_string(), Value::Bool(false));
            let mut tool_value = Value::Object(mapped);
            lowercase_type_fields(&mut tool_value);
            out.push(tool_value);
        }
    }
    Some(out)
}

fn clean_gemini_tool_schema(value: &Value) -> Value {
    let mut schema = value.clone();
    if let Value::Object(obj) = &mut schema {
        obj.remove("$schema");
        obj.insert("additionalProperties".to_string(), Value::Bool(false));
    }
    schema
}

fn lowercase_type_fields(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                lowercase_type_fields(item);
            }
        }
        Value::Object(obj) => {
            if let Some(Value::String(text)) = obj.get_mut("type") {
                *text = text.to_ascii_lowercase();
            }
            for value in obj.values_mut() {
                lowercase_type_fields(value);
            }
        }
        _ => {}
    }
}

fn resolve_gemini_cpa_reasoning_effort(source: &serde_json::Map<String, Value>) -> String {
    let mut effort: Option<String> = None;
    if let Some(gen_config) = get_object_field(source, &["generationConfig"]) {
        if let Some(thinking_config) = get_object_field(gen_config, &["thinkingConfig"]) {
            if let Some(level) =
                get_value_field(thinking_config, &["thinkingLevel", "thinking_level"])
                    .and_then(Value::as_str)
            {
                let normalized = level.trim().to_ascii_lowercase();
                if !normalized.is_empty() {
                    effort = Some(normalized);
                }
            } else if let Some(budget) =
                get_value_field(thinking_config, &["thinkingBudget", "thinking_budget"])
                    .and_then(Value::as_i64)
            {
                if let Some(mapped) = gemini_cpa_budget_to_level(budget) {
                    effort = Some(mapped.to_string());
                }
            }
        }
    }
    effort.unwrap_or_else(|| "medium".to_string())
}

fn gemini_cpa_budget_to_level(budget: i64) -> Option<&'static str> {
    match budget {
        i64::MIN..=-2 => None,
        -1 => Some("auto"),
        0 => Some("none"),
        1..=512 => Some("minimal"),
        513..=1024 => Some("low"),
        1025..=8192 => Some("medium"),
        8193..=24576 => Some("high"),
        24577..=i64::MAX => Some("xhigh"),
    }
}

fn normalized_request_path(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

fn extract_model_from_path(path: &str) -> Option<String> {
    let normalized = normalized_request_path(path);
    ["/v1/models/", "/v1beta/models/", "/v1alpha/models/"]
        .iter()
        .find_map(|prefix| {
            normalized.strip_prefix(prefix).and_then(|rest| {
                let (model, _) = rest.split_once(':')?;
                let trimmed = model.trim();
                if trimmed.is_empty() {
                    None
                } else if let Some(stripped) = trimmed.strip_prefix("models/") {
                    Some(stripped.to_string())
                } else {
                    Some(trimmed.to_string())
                }
            })
        })
}

fn get_value_field<'a>(
    source: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<&'a Value> {
    keys.iter().find_map(|key| source.get(*key))
}

fn get_array_field<'a>(
    source: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<&'a Vec<Value>> {
    get_value_field(source, keys).and_then(Value::as_array)
}

fn get_object_field<'a>(
    source: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    get_value_field(source, keys).and_then(Value::as_object)
}
