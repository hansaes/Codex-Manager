use crate::apikey_profile::is_gemini_generate_content_request_path;

use super::{request_mapping, AdaptedGatewayRequest, GeminiStreamOutputMode, ResponseAdapter};

pub(super) fn adapt_gemini_request(
    path: &str,
    body: &[u8],
) -> Result<Option<AdaptedGatewayRequest>, String> {
    if !is_gemini_generate_content_request_path(path) {
        return Ok(None);
    }

    let (adapted_body, request_stream, tool_name_restore_map) =
        request_mapping::convert_gemini_generate_content_request(path, &body)?;
    let normalized_path = normalized_request_path(path);
    let response_adapter = if normalized_path.starts_with("/v1internal:") {
        if request_stream {
            ResponseAdapter::GeminiCliSse
        } else {
            ResponseAdapter::GeminiCliJson
        }
    } else if request_stream {
        ResponseAdapter::GeminiSse
    } else {
        ResponseAdapter::GeminiJson
    };

    Ok(Some(AdaptedGatewayRequest {
        path: "/v1/responses".to_string(),
        body: adapted_body,
        response_adapter,
        gemini_stream_output_mode: resolve_gemini_stream_output_mode(path, request_stream),
        tool_name_restore_map,
    }))
}

fn resolve_gemini_stream_output_mode(
    path: &str,
    request_stream: bool,
) -> Option<GeminiStreamOutputMode> {
    if !request_stream {
        return None;
    }
    let query = path
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default();
    for item in query.split('&') {
        let Some((key, value)) = item.split_once('=') else {
            continue;
        };
        if !key.eq_ignore_ascii_case("alt") {
            continue;
        }
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() || normalized == "sse" {
            return Some(GeminiStreamOutputMode::Sse);
        }
        return Some(GeminiStreamOutputMode::Raw);
    }
    Some(GeminiStreamOutputMode::Sse)
}

fn normalized_request_path(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}
