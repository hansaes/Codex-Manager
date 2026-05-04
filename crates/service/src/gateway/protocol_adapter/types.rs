use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseAdapter {
    Passthrough,
    AnthropicJson,
    AnthropicSse,
    GeminiJson,
    GeminiSse,
    GeminiCliJson,
    GeminiCliSse,
    OpenAIResponsesJsonFromChatCompletions,
    OpenAIResponsesSseFromChatCompletions,
    OpenAIResponsesJsonFromAnthropic,
    OpenAIResponsesSseFromAnthropic,
    OpenAIChatCompletionsJson,
    OpenAIChatCompletionsSse,
    OpenAICompletionsJson,
    OpenAICompletionsSse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeminiStreamOutputMode {
    Sse,
    Raw,
}

pub(crate) type ToolNameRestoreMap = BTreeMap<String, String>;

#[derive(Debug)]
pub(crate) struct AdaptedGatewayRequest {
    pub(crate) path: String,
    pub(crate) body: Vec<u8>,
    pub(crate) response_adapter: ResponseAdapter,
    pub(crate) gemini_stream_output_mode: Option<GeminiStreamOutputMode>,
    pub(crate) tool_name_restore_map: ToolNameRestoreMap,
}
