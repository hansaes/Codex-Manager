use crate::apikey_profile::{
    PROTOCOL_ANTHROPIC_NATIVE, PROTOCOL_GEMINI_NATIVE, PROTOCOL_OPENAI_COMPAT,
};

use super::{AdaptedGatewayRequest, ResponseAdapter, ToolNameRestoreMap};

/// 函数 `adapt_request_for_protocol`
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
pub(crate) fn adapt_request_for_protocol(
    protocol_type: &str,
    path: &str,
    body: Vec<u8>,
) -> Result<AdaptedGatewayRequest, String> {
    if protocol_type == PROTOCOL_OPENAI_COMPAT {
        if let Some(adapted) = super::codex_adapter::adapt_openai_compat_request(path, &body)? {
            return Ok(adapted);
        }
    }

    if protocol_type == PROTOCOL_ANTHROPIC_NATIVE {
        if let Some(adapted) = super::claude_adapter::adapt_anthropic_request(path, &body)? {
            return Ok(adapted);
        }
    }

    if protocol_type == PROTOCOL_GEMINI_NATIVE {
        if let Some(adapted) = super::gemini_adapter::adapt_gemini_request(path, &body)? {
            return Ok(adapted);
        }
    }

    Ok(AdaptedGatewayRequest {
        path: path.to_string(),
        body,
        response_adapter: ResponseAdapter::Passthrough,
        gemini_stream_output_mode: None,
        tool_name_restore_map: ToolNameRestoreMap::new(),
    })
}
