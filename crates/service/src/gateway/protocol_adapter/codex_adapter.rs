use super::{request_mapping, AdaptedGatewayRequest, ResponseAdapter};

pub(super) fn adapt_openai_compat_request(
    path: &str,
    body: &[u8],
) -> Result<Option<AdaptedGatewayRequest>, String> {
    if path == "/v1/chat/completions" || path.starts_with("/v1/chat/completions?") {
        let (adapted_body, request_stream, tool_name_restore_map) =
            request_mapping::convert_openai_chat_completions_request(&body)?;
        return Ok(Some(AdaptedGatewayRequest {
            path: rewrite_responses_path(path, "/v1/chat/completions"),
            body: adapted_body,
            response_adapter: if request_stream {
                ResponseAdapter::OpenAIChatCompletionsSse
            } else {
                ResponseAdapter::OpenAIChatCompletionsJson
            },
            gemini_stream_output_mode: None,
            tool_name_restore_map,
        }));
    }

    if path == "/v1/completions" || path.starts_with("/v1/completions?") {
        let (chat_body, _) = request_mapping::convert_openai_completions_request(body)?;
        let (adapted_body, request_stream, tool_name_restore_map) =
            request_mapping::convert_openai_chat_completions_request(&chat_body)?;
        return Ok(Some(AdaptedGatewayRequest {
            path: rewrite_responses_path(path, "/v1/completions"),
            body: adapted_body,
            response_adapter: if request_stream {
                ResponseAdapter::OpenAICompletionsSse
            } else {
                ResponseAdapter::OpenAICompletionsJson
            },
            gemini_stream_output_mode: None,
            tool_name_restore_map,
        }));
    }
    Ok(None)
}

fn rewrite_responses_path(path: &str, prefix: &str) -> String {
    if let Some(suffix) = path.strip_prefix(prefix) {
        format!("/v1/responses{suffix}")
    } else {
        "/v1/responses".to_string()
    }
}
