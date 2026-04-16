use super::{request_mapping, AdaptedGatewayRequest, ResponseAdapter};

pub(super) fn adapt_anthropic_request(
    path: &str,
    body: &[u8],
) -> Result<Option<AdaptedGatewayRequest>, String> {
    if path == "/v1/messages" || path.starts_with("/v1/messages?") {
        let (adapted_body, request_stream, tool_name_restore_map) =
            request_mapping::convert_anthropic_messages_request(&body)?;
        return Ok(Some(AdaptedGatewayRequest {
            // 说明：non-stream 也统一走 /v1/responses。
            // 在部分账号/环境下 /v1/responses/compact 更容易触发 challenge 或非预期拦截。
            path: "/v1/responses".to_string(),
            body: adapted_body,
            response_adapter: if request_stream {
                ResponseAdapter::AnthropicSse
            } else {
                ResponseAdapter::AnthropicJson
            },
            gemini_stream_output_mode: None,
            tool_name_restore_map,
        }));
    }

    Ok(None)
}
