use super::{
    classify_upstream_stream_read_error, collector_output_text_trimmed, mark_first_response_ms,
    merge_usage, parse_sse_frame_json, stream_idle_timed_out, stream_idle_timeout_message,
    stream_reader_disconnected_message, stream_wait_timeout,
    upstream_hint_or_stream_incomplete_message, update_openai_stream_meta, Arc, Cursor, Mutex,
    OpenAIResponsesEvent, OpenAIStreamMeta, PassthroughSseCollector, Read, SseKeepAliveFrame,
    SseTerminal, ToolNameRestoreMap, Value, extract_sse_frame_payload,
};
use crate::gateway::upstream::{GatewayByteStream, GatewayByteStreamItem, GatewayStreamResponse};
use crate::gateway::{
    adapt_upstream_response_with_tool_name_restore_map,
    convert_openai_chat_stream_chunk_with_tool_name_restore_map, ResponseAdapter,
};
use eventsource_stream::{Event, Eventsource};
use futures_util::pin_mut;
use futures_util::stream::unfold;
use futures_util::task::noop_waker_ref;
use futures_util::Stream;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::task::{Context, Poll};
use std::thread;
use std::time::{Duration, Instant};

const OPENAI_RESPONSES_BRIDGE_FRAME_CHANNEL_CAPACITY: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenAIResponsesBridgeSource {
    ChatCompletions,
    AnthropicNative,
}

#[derive(Debug)]
enum EventsourceFrameItem {
    Frame(Vec<String>),
    Eof,
    Error(String),
}

struct EventsourceFramePump {
    rx: Receiver<EventsourceFrameItem>,
}

impl EventsourceFramePump {
    fn new(byte_stream: GatewayByteStream) -> Self {
        let (tx, rx) =
            mpsc::sync_channel::<EventsourceFrameItem>(OPENAI_RESPONSES_BRIDGE_FRAME_CHANNEL_CAPACITY);
        thread::spawn(move || {
            let byte_stream = unfold(Some(byte_stream), |state| async move {
                let byte_stream = state?;
                match byte_stream.recv() {
                    Ok(GatewayByteStreamItem::Chunk(bytes)) => Some((Ok(bytes), Some(byte_stream))),
                    Ok(GatewayByteStreamItem::Eof) => None,
                    Ok(GatewayByteStreamItem::Error(err)) => Some((Err(err), None)),
                    Err(_) => None,
                }
            });

            let stream = byte_stream.eventsource();
            pin_mut!(stream);
            let waker = noop_waker_ref();
            let mut cx = Context::from_waker(waker);

            loop {
                match stream.as_mut().poll_next(&mut cx) {
                    Poll::Ready(Some(Ok(event))) => {
                        if tx.send(EventsourceFrameItem::Frame(event_to_sse_lines(&event))).is_err()
                        {
                            return;
                        }
                    }
                    Poll::Ready(Some(Err(err))) => {
                        let _ = tx.send(EventsourceFrameItem::Error(err.to_string()));
                        return;
                    }
                    Poll::Ready(None) => {
                        let _ = tx.send(EventsourceFrameItem::Eof);
                        return;
                    }
                    Poll::Pending => thread::yield_now(),
                }
            }
        });
        Self { rx }
    }

    fn recv_timeout(&self, timeout: Duration) -> Result<EventsourceFrameItem, RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }
}

fn event_to_sse_lines(event: &Event) -> Vec<String> {
    let mut lines = Vec::new();
    if !event.id.is_empty() {
        lines.push(format!("id: {}\n", event.id));
    }
    if let Some(retry) = event.retry {
        lines.push(format!("retry: {}\n", retry.as_millis()));
    }
    if !event.event.is_empty() && !event.event.eq_ignore_ascii_case("message") {
        lines.push(format!("event: {}\n", event.event));
    }
    for data_line in event.data.split('\n') {
        lines.push(format!("data: {data_line}\n"));
    }
    lines.push("\n".to_string());
    lines
}

#[derive(Debug, Clone, Default)]
struct PendingFunctionCall {
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
    added_emitted: bool,
}

fn merge_tool_call_arguments(existing: &mut String, fragment: &str) {
    if fragment.is_empty() {
        return;
    }
    if existing.is_empty() {
        existing.push_str(fragment);
        return;
    }
    if existing == fragment || existing.ends_with(fragment) || existing.starts_with(fragment) {
        return;
    }
    if fragment.starts_with(existing.as_str()) {
        *existing = fragment.to_string();
        return;
    }
    existing.push_str(fragment);
}

fn restore_tool_name(
    raw_name: &str,
    tool_name_restore_map: Option<&ToolNameRestoreMap>,
) -> String {
    tool_name_restore_map
        .and_then(|map| map.get(raw_name))
        .cloned()
        .unwrap_or_else(|| raw_name.to_string())
}

fn extract_chat_delta_text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    let mut out = String::new();
    let Some(parts) = value.as_array() else {
        return out;
    };
    for part in parts {
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            out.push_str(text);
        }
    }
    out
}

fn build_responses_usage_json(collector: &PassthroughSseCollector) -> Option<Value> {
    let mut usage = serde_json::Map::new();
    if let Some(value) = collector.usage.input_tokens {
        usage.insert("input_tokens".to_string(), Value::from(value));
    }
    if let Some(value) = collector.usage.output_tokens {
        usage.insert("output_tokens".to_string(), Value::from(value));
    }
    if let Some(value) = collector.usage.total_tokens {
        usage.insert("total_tokens".to_string(), Value::from(value));
    }
    if let Some(value) = collector.usage.cached_input_tokens {
        usage.insert(
            "input_tokens_details".to_string(),
            json!({ "cached_tokens": value }),
        );
    }
    if let Some(value) = collector.usage.reasoning_output_tokens {
        usage.insert(
            "output_tokens_details".to_string(),
            json!({ "reasoning_tokens": value }),
        );
    }
    if usage.is_empty() {
        None
    } else {
        Some(Value::Object(usage))
    }
}

pub(crate) struct OpenAIResponsesBridgeSseReader {
    upstream: EventsourceFramePump,
    out_cursor: Cursor<Vec<u8>>,
    usage_collector: Arc<Mutex<PassthroughSseCollector>>,
    source: OpenAIResponsesBridgeSource,
    tool_name_restore_map: Option<ToolNameRestoreMap>,
    stream_meta: OpenAIStreamMeta,
    raw_sse: Vec<u8>,
    pending_tool_calls: BTreeMap<i64, PendingFunctionCall>,
    emitted_text_delta: bool,
    request_started_at: Instant,
    last_upstream_activity: Instant,
    saw_upstream_frame: bool,
    finished: bool,
}

impl OpenAIResponsesBridgeSseReader {
    pub(crate) fn new_chat_completions(
        upstream: reqwest::blocking::Response,
        usage_collector: Arc<Mutex<PassthroughSseCollector>>,
        tool_name_restore_map: Option<ToolNameRestoreMap>,
        request_started_at: Instant,
    ) -> Self {
        Self::from_stream_response(
            OpenAIResponsesBridgeSource::ChatCompletions,
            GatewayStreamResponse::from_blocking_response(upstream),
            usage_collector,
            tool_name_restore_map,
            request_started_at,
        )
    }

    pub(crate) fn new_chat_completions_from_stream_response(
        upstream: GatewayStreamResponse,
        usage_collector: Arc<Mutex<PassthroughSseCollector>>,
        tool_name_restore_map: Option<ToolNameRestoreMap>,
        request_started_at: Instant,
    ) -> Self {
        Self::from_stream_response(
            OpenAIResponsesBridgeSource::ChatCompletions,
            upstream,
            usage_collector,
            tool_name_restore_map,
            request_started_at,
        )
    }

    pub(crate) fn new_anthropic(
        upstream: reqwest::blocking::Response,
        usage_collector: Arc<Mutex<PassthroughSseCollector>>,
        tool_name_restore_map: Option<ToolNameRestoreMap>,
        request_started_at: Instant,
    ) -> Self {
        Self::from_stream_response(
            OpenAIResponsesBridgeSource::AnthropicNative,
            GatewayStreamResponse::from_blocking_response(upstream),
            usage_collector,
            tool_name_restore_map,
            request_started_at,
        )
    }

    pub(crate) fn new_anthropic_from_stream_response(
        upstream: GatewayStreamResponse,
        usage_collector: Arc<Mutex<PassthroughSseCollector>>,
        tool_name_restore_map: Option<ToolNameRestoreMap>,
        request_started_at: Instant,
    ) -> Self {
        Self::from_stream_response(
            OpenAIResponsesBridgeSource::AnthropicNative,
            upstream,
            usage_collector,
            tool_name_restore_map,
            request_started_at,
        )
    }

    fn from_stream_response(
        source: OpenAIResponsesBridgeSource,
        upstream: GatewayStreamResponse,
        usage_collector: Arc<Mutex<PassthroughSseCollector>>,
        tool_name_restore_map: Option<ToolNameRestoreMap>,
        request_started_at: Instant,
    ) -> Self {
        Self {
            upstream: EventsourceFramePump::new(upstream.into_body()),
            out_cursor: Cursor::new(Vec::new()),
            usage_collector,
            source,
            tool_name_restore_map,
            stream_meta: OpenAIStreamMeta::default(),
            raw_sse: Vec::new(),
            pending_tool_calls: BTreeMap::new(),
            emitted_text_delta: false,
            request_started_at,
            last_upstream_activity: Instant::now(),
            saw_upstream_frame: false,
            finished: false,
        }
    }

    fn next_chunk(&mut self) -> std::io::Result<Vec<u8>> {
        loop {
            match self
                .upstream
                .recv_timeout(stream_wait_timeout(self.last_upstream_activity))
            {
                Ok(EventsourceFrameItem::Frame(frame)) => {
                    self.last_upstream_activity = Instant::now();
                    self.saw_upstream_frame = true;
                    self.append_raw_frame(&frame);
                    let mapped = match self.source {
                        OpenAIResponsesBridgeSource::ChatCompletions => {
                            self.map_chat_frame_to_responses_sse(&frame)?
                        }
                        OpenAIResponsesBridgeSource::AnthropicNative => {
                            self.map_anthropic_frame_to_responses_sse(&frame)?
                        }
                    };
                    if !mapped.is_empty() {
                        mark_first_response_ms(&self.usage_collector, self.request_started_at);
                        return Ok(mapped);
                    }
                    continue;
                }
                Ok(EventsourceFrameItem::Eof) => {
                    self.last_upstream_activity = Instant::now();
                    if let Ok(mut collector) = self.usage_collector.lock() {
                        if !collector.saw_terminal {
                            let hint = collector.upstream_error_hint.clone();
                            collector.terminal_error.get_or_insert_with(|| {
                                upstream_hint_or_stream_incomplete_message(hint.as_deref())
                            });
                        }
                    }
                    self.finished = true;
                    return Ok(Vec::new());
                }
                Ok(EventsourceFrameItem::Error(err)) => {
                    self.last_upstream_activity = Instant::now();
                    if let Ok(mut collector) = self.usage_collector.lock() {
                        collector
                            .terminal_error
                            .get_or_insert_with(|| classify_upstream_stream_read_error(&err));
                    }
                    self.finished = true;
                    return Ok(Vec::new());
                }
                Err(RecvTimeoutError::Timeout) => {
                    if stream_idle_timed_out(self.last_upstream_activity) {
                        if let Ok(mut collector) = self.usage_collector.lock() {
                            collector
                                .terminal_error
                                .get_or_insert_with(stream_idle_timeout_message);
                        }
                        self.finished = true;
                        return Ok(Vec::new());
                    }
                    if self.saw_upstream_frame {
                        return Ok(SseKeepAliveFrame::OpenAIResponses.bytes().to_vec());
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    if let Ok(mut collector) = self.usage_collector.lock() {
                        let hint = collector.upstream_error_hint.clone();
                        collector.terminal_error.get_or_insert_with(|| {
                            hint.unwrap_or_else(stream_reader_disconnected_message)
                        });
                    }
                    self.finished = true;
                    return Ok(Vec::new());
                }
            }
        }
    }

    fn append_raw_frame(&mut self, lines: &[String]) {
        for line in lines {
            self.raw_sse.extend_from_slice(line.as_bytes());
        }
    }

    fn update_usage_from_emitted_frame(&self, lines: &[String]) {
        let Some(event) = OpenAIResponsesEvent::parse(lines) else {
            return;
        };
        if let Ok(mut collector) = self.usage_collector.lock() {
            if let Some(event_type) = event.event_type {
                collector.last_event_type = Some(event_type);
            }
            merge_usage(&mut collector.usage, event.usage);
            if let Some(upstream_error_hint) = event.upstream_error_hint {
                collector.upstream_error_hint = Some(upstream_error_hint);
            }
            if let Some(terminal) = event.terminal {
                collector.saw_terminal = true;
                if let SseTerminal::Err(message) = terminal {
                    collector.terminal_error = Some(message);
                } else {
                    collector.terminal_error = None;
                }
            }
        }
    }

    fn append_emitted_event(out: &mut String, lines: &[String]) {
        for line in lines {
            out.push_str(line);
        }
    }

    fn build_event_lines(event_name: &str, payload: &Value) -> Vec<String> {
        let data = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
        vec![
            format!("event: {event_name}\n"),
            format!("data: {data}\n"),
            "\n".to_string(),
        ]
    }

    fn push_event(&self, out: &mut String, event_name: &str, payload: &Value) {
        let lines = Self::build_event_lines(event_name, payload);
        self.update_usage_from_emitted_frame(&lines);
        Self::append_emitted_event(out, &lines);
    }

    fn emit_failure_stream(&mut self, message: impl Into<String>) -> Vec<u8> {
        let message = message.into();
        let mut out = String::new();
        let payload = json!({
            "type": "response.failed",
            "error": {
                "message": message,
                "type": "server_error"
            }
        });
        self.push_event(&mut out, "response.failed", &payload);
        out.push_str("data: [DONE]\n\n");
        self.finished = true;
        out.into_bytes()
    }

    fn map_chat_frame_to_responses_sse(&mut self, lines: &[String]) -> std::io::Result<Vec<u8>> {
        let Some(payload) = extract_sse_frame_payload(lines) else {
            return Ok(Vec::new());
        };
        if payload.trim() == "[DONE]" {
            return Ok(self.finish_success_stream());
        }

        let Some(value) = parse_sse_frame_json(lines) else {
            return Ok(Vec::new());
        };
        if value.get("error").is_some() {
            let message = value
                .get("error")
                .and_then(Value::as_object)
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("upstream request failed");
            return Ok(self.emit_failure_stream(message));
        }

        let mapped = convert_openai_chat_stream_chunk_with_tool_name_restore_map(
            &value,
            self.tool_name_restore_map.as_ref(),
        )
        .unwrap_or_else(|| value.clone());
        update_openai_stream_meta(&mut self.stream_meta, &mapped);

        let mut out = String::new();
        if let Some(choices) = mapped.get("choices").and_then(Value::as_array) {
            for (choice_idx, choice) in choices.iter().enumerate() {
                if let Some(content) = choice.get("delta").and_then(|delta| delta.get("content")) {
                    let delta_text = extract_chat_delta_text(content);
                    if !delta_text.is_empty() {
                        let payload = json!({
                            "type": "response.output_text.delta",
                            "response_id": self.stream_meta.response_id.clone().unwrap_or_default(),
                            "created": self.stream_meta.created.unwrap_or(0),
                            "model": self.stream_meta.model.clone().unwrap_or_default(),
                            "delta": delta_text,
                        });
                        self.push_event(&mut out, "response.output_text.delta", &payload);
                        self.emitted_text_delta = true;
                    }
                }
                let Some(tool_calls) = choice
                    .get("delta")
                    .and_then(|delta| delta.get("tool_calls"))
                    .and_then(Value::as_array)
                else {
                    continue;
                };
                for (position, tool_call) in tool_calls.iter().enumerate() {
                    let Some(tool_obj) = tool_call.as_object() else {
                        continue;
                    };
                    let output_index = tool_obj
                        .get("index")
                        .and_then(Value::as_i64)
                        .unwrap_or((choice_idx + position) as i64);
                    if let Some(function) = tool_obj.get("function").and_then(Value::as_object) {
                        let mut added_payload: Option<Value> = None;
                        {
                            let entry = self.pending_tool_calls.entry(output_index).or_default();
                            if let Some(call_id) = tool_obj
                                .get("id")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                            {
                                entry.call_id = Some(call_id.to_string());
                            }
                            if let Some(name) = function
                                .get("name")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                            {
                                entry.name = Some(name.to_string());
                            }
                            if !entry.added_emitted
                                && (entry.call_id.is_some() || entry.name.is_some())
                            {
                                let item = json!({
                                    "type": "function_call",
                                    "call_id": entry.call_id.clone().unwrap_or_else(|| format!("call_{output_index}")),
                                    "name": entry.name.clone().unwrap_or_else(|| "tool".to_string()),
                                });
                                added_payload = Some(json!({
                                    "type": "response.output_item.added",
                                    "response_id": self.stream_meta.response_id.clone().unwrap_or_default(),
                                    "created": self.stream_meta.created.unwrap_or(0),
                                    "model": self.stream_meta.model.clone().unwrap_or_default(),
                                    "output_index": output_index,
                                    "item": item,
                                }));
                                entry.added_emitted = true;
                            }
                        }
                        if let Some(payload) = added_payload {
                            self.push_event(&mut out, "response.output_item.added", &payload);
                        }
                        if let Some(arguments) = function
                            .get("arguments")
                            .and_then(Value::as_str)
                            .filter(|value| !value.is_empty())
                        {
                            let item_id = {
                                let entry = self.pending_tool_calls.entry(output_index).or_default();
                                merge_tool_call_arguments(&mut entry.arguments, arguments);
                                entry.call_id.clone().unwrap_or_default()
                            };
                            let payload = json!({
                                "type": "response.function_call_arguments.delta",
                                "response_id": self.stream_meta.response_id.clone().unwrap_or_default(),
                                "created": self.stream_meta.created.unwrap_or(0),
                                "model": self.stream_meta.model.clone().unwrap_or_default(),
                                "output_index": output_index,
                                "item_id": item_id,
                                "delta": arguments,
                            });
                            self.push_event(
                                &mut out,
                                "response.function_call_arguments.delta",
                                &payload,
                            );
                        }
                    }
                }
            }
        }

        Ok(out.into_bytes())
    }

    fn map_anthropic_frame_to_responses_sse(
        &mut self,
        lines: &[String],
    ) -> std::io::Result<Vec<u8>> {
        let Some(value) = parse_sse_frame_json(lines) else {
            return Ok(Vec::new());
        };
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if event_type == "error" {
            let message = value
                .get("error")
                .and_then(Value::as_object)
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("upstream request failed");
            return Ok(self.emit_failure_stream(message));
        }

        self.capture_anthropic_meta(&value);

        let mut out = String::new();
        match event_type {
            "content_block_delta" => {
                let Some(delta) = value.get("delta").and_then(Value::as_object) else {
                    return Ok(Vec::new());
                };
                let delta_type = delta
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if delta_type == "text_delta" {
                    let fragment = delta
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if !fragment.is_empty() {
                        let payload = json!({
                            "type": "response.output_text.delta",
                            "response_id": self.stream_meta.response_id.clone().unwrap_or_default(),
                            "created": self.stream_meta.created.unwrap_or(0),
                            "model": self.stream_meta.model.clone().unwrap_or_default(),
                            "delta": fragment,
                        });
                        self.push_event(&mut out, "response.output_text.delta", &payload);
                        self.emitted_text_delta = true;
                    }
                } else if delta_type == "input_json_delta" {
                    let output_index = value.get("index").and_then(Value::as_i64).unwrap_or(0);
                    let fragment = delta
                        .get("partial_json")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if !fragment.is_empty() {
                        let mut added_payload: Option<Value> = None;
                        let item_id = {
                            let entry = self.pending_tool_calls.entry(output_index).or_default();
                            merge_tool_call_arguments(&mut entry.arguments, fragment);
                            if !entry.added_emitted
                                && (entry.call_id.is_some() || entry.name.is_some())
                            {
                                let item = json!({
                                    "type": "function_call",
                                    "call_id": entry.call_id.clone().unwrap_or_else(|| format!("call_{output_index}")),
                                    "name": entry.name.clone().unwrap_or_else(|| "tool".to_string()),
                                });
                                added_payload = Some(json!({
                                    "type": "response.output_item.added",
                                    "response_id": self.stream_meta.response_id.clone().unwrap_or_default(),
                                    "created": self.stream_meta.created.unwrap_or(0),
                                    "model": self.stream_meta.model.clone().unwrap_or_default(),
                                    "output_index": output_index,
                                    "item": item,
                                }));
                                entry.added_emitted = true;
                            }
                            entry.call_id.clone().unwrap_or_default()
                        };
                        if let Some(payload) = added_payload {
                            self.push_event(&mut out, "response.output_item.added", &payload);
                        }
                        let payload = json!({
                            "type": "response.function_call_arguments.delta",
                            "response_id": self.stream_meta.response_id.clone().unwrap_or_default(),
                            "created": self.stream_meta.created.unwrap_or(0),
                            "model": self.stream_meta.model.clone().unwrap_or_default(),
                            "output_index": output_index,
                            "item_id": item_id,
                            "delta": fragment,
                        });
                        self.push_event(
                            &mut out,
                            "response.function_call_arguments.delta",
                            &payload,
                        );
                    }
                }
            }
            "content_block_start" => {
                let Some(content_block) = value.get("content_block").and_then(Value::as_object)
                else {
                    return Ok(Vec::new());
                };
                if content_block
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind == "tool_use")
                {
                    let output_index = value.get("index").and_then(Value::as_i64).unwrap_or(0);
                    let mut added_payload: Option<Value> = None;
                    {
                        let entry = self.pending_tool_calls.entry(output_index).or_default();
                        if let Some(call_id) = content_block
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            entry.call_id = Some(call_id.to_string());
                        }
                        if let Some(name) = content_block
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            entry.name =
                                Some(restore_tool_name(name, self.tool_name_restore_map.as_ref()));
                        }
                        if !entry.added_emitted {
                            let item = json!({
                                "type": "function_call",
                                "call_id": entry.call_id.clone().unwrap_or_else(|| format!("call_{output_index}")),
                                "name": entry.name.clone().unwrap_or_else(|| "tool".to_string()),
                            });
                            added_payload = Some(json!({
                                "type": "response.output_item.added",
                                "response_id": self.stream_meta.response_id.clone().unwrap_or_default(),
                                "created": self.stream_meta.created.unwrap_or(0),
                                "model": self.stream_meta.model.clone().unwrap_or_default(),
                                "output_index": output_index,
                                "item": item,
                            }));
                            entry.added_emitted = true;
                        }
                    }
                    if let Some(payload) = added_payload {
                        self.push_event(&mut out, "response.output_item.added", &payload);
                    }
                }
            }
            "message_stop" => return Ok(self.finish_success_stream()),
            _ => {}
        }

        Ok(out.into_bytes())
    }

    fn capture_anthropic_meta(&mut self, value: &Value) {
        if let Some(message) = value.get("message").and_then(Value::as_object) {
            if self.stream_meta.response_id.is_none() {
                self.stream_meta.response_id = message
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if self.stream_meta.model.is_none() {
                self.stream_meta.model = message
                    .get("model")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
        }
    }

    fn finish_success_stream(&mut self) -> Vec<u8> {
        let completed_response = self
            .build_completed_response_value()
            .unwrap_or_else(|_| self.build_fallback_completed_response());
        let mut out = String::new();
        if !self.emitted_text_delta {
            if let Some(text) = completed_response
                .get("output_text")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let payload = json!({
                    "type": "response.output_text.delta",
                    "response_id": completed_response
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("resp_proxy"),
                    "created": completed_response.get("created").and_then(Value::as_i64).unwrap_or(0),
                    "model": completed_response.get("model").and_then(Value::as_str).unwrap_or("unknown"),
                    "delta": text,
                });
                self.push_event(&mut out, "response.output_text.delta", &payload);
                self.emitted_text_delta = true;
            }
        }
        self.emit_final_function_call_frames(&mut out, &completed_response);
        let payload = json!({
            "type": "response.completed",
            "response": completed_response,
        });
        self.push_event(&mut out, "response.completed", &payload);
        out.push_str("data: [DONE]\n\n");
        self.finished = true;
        out.into_bytes()
    }

    fn build_completed_response_value(&self) -> Result<Value, String> {
        let adapter = match self.source {
            OpenAIResponsesBridgeSource::ChatCompletions => {
                ResponseAdapter::OpenAIResponsesJsonFromChatCompletions
            }
            OpenAIResponsesBridgeSource::AnthropicNative => {
                ResponseAdapter::OpenAIResponsesJsonFromAnthropic
            }
        };
        let (body, _) = adapt_upstream_response_with_tool_name_restore_map(
            adapter,
            Some("text/event-stream"),
            self.raw_sse.as_slice(),
            self.tool_name_restore_map.as_ref(),
        )?;
        serde_json::from_slice::<Value>(&body)
            .map_err(|err| format!("parse synthesized responses json failed: {err}"))
    }

    fn build_fallback_completed_response(&self) -> Value {
        let collector = self
            .usage_collector
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let mut output = Vec::new();
        if let Some(text) = collector_output_text_trimmed(&self.usage_collector) {
            output.push(json!({
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": text,
                }]
            }));
        }
        for (index, call) in &self.pending_tool_calls {
            if call.call_id.is_none() && call.name.is_none() && call.arguments.trim().is_empty() {
                continue;
            }
            output.push(json!({
                "type": "function_call",
                "index": index,
                "call_id": call.call_id.clone().unwrap_or_else(|| format!("call_{index}")),
                "name": call.name.clone().unwrap_or_else(|| "tool".to_string()),
                "arguments": call.arguments,
            }));
        }
        let mut out = serde_json::Map::new();
        out.insert(
            "id".to_string(),
            Value::String(
                self.stream_meta
                    .response_id
                    .clone()
                    .unwrap_or_else(|| "resp_proxy".to_string()),
            ),
        );
        out.insert("object".to_string(), Value::String("response".to_string()));
        out.insert(
            "created".to_string(),
            Value::Number(self.stream_meta.created.unwrap_or(0).into()),
        );
        out.insert(
            "model".to_string(),
            Value::String(
                self.stream_meta
                    .model
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
        );
        out.insert("status".to_string(), Value::String("completed".to_string()));
        out.insert("output".to_string(), Value::Array(output));
        if let Some(text) = collector.usage.output_text.as_deref().map(str::trim) {
            if !text.is_empty() {
                out.insert("output_text".to_string(), Value::String(text.to_string()));
            }
        }
        if let Some(usage) = build_responses_usage_json(&collector) {
            out.insert("usage".to_string(), usage);
        }
        Value::Object(out)
    }

    fn emit_final_function_call_frames(&mut self, out: &mut String, response: &Value) {
        let Some(output_items) = response.get("output").and_then(Value::as_array) else {
            return;
        };
        let response_id = response
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("resp_proxy");
        let created = response.get("created").and_then(Value::as_i64).unwrap_or(0);
        let model = response
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        for (fallback_index, item) in output_items.iter().enumerate() {
            let Some(item_obj) = item.as_object() else {
                continue;
            };
            let item_type = item_obj
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !matches!(item_type, "function_call" | "custom_tool_call") {
                continue;
            }
            let output_index = item_obj
                .get("index")
                .and_then(Value::as_i64)
                .unwrap_or(fallback_index as i64);
            let mut added_payload: Option<Value> = None;
            let mut done_args: Option<String> = None;
            {
                let entry = self.pending_tool_calls.entry(output_index).or_default();
                if let Some(call_id) = item_obj
                    .get("call_id")
                    .or_else(|| item_obj.get("id"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    entry.call_id = Some(call_id.to_string());
                }
                if let Some(name) = item_obj
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    entry.name = Some(name.to_string());
                }
                if let Some(arguments) = item_obj.get("arguments").and_then(Value::as_str) {
                    merge_tool_call_arguments(&mut entry.arguments, arguments);
                } else if let Some(arguments) = item_obj.get("input") {
                    let serialized =
                        serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string());
                    merge_tool_call_arguments(&mut entry.arguments, serialized.as_str());
                }

                if !entry.added_emitted {
                    let added_item = json!({
                        "type": "function_call",
                        "call_id": entry.call_id.clone().unwrap_or_else(|| format!("call_{output_index}")),
                        "name": entry.name.clone().unwrap_or_else(|| "tool".to_string()),
                    });
                    added_payload = Some(json!({
                        "type": "response.output_item.added",
                        "response_id": response_id,
                        "created": created,
                        "model": model,
                        "output_index": output_index,
                        "item": added_item,
                    }));
                    entry.added_emitted = true;
                }

                if !entry.arguments.is_empty() {
                    done_args = Some(entry.arguments.clone());
                }
            }

            if let Some(payload) = added_payload {
                self.push_event(out, "response.output_item.added", &payload);
            }

            if let Some(arguments) = done_args {
                let payload = json!({
                    "type": "response.function_call_arguments.done",
                    "response_id": response_id,
                    "created": created,
                    "model": model,
                    "output_index": output_index,
                    "item_id": self
                        .pending_tool_calls
                        .get(&output_index)
                        .and_then(|entry| entry.call_id.clone())
                        .unwrap_or_default(),
                    "arguments": arguments,
                });
                self.push_event(out, "response.function_call_arguments.done", &payload);
            }

            let payload = json!({
                "type": "response.output_item.done",
                "response_id": response_id,
                "created": created,
                "model": model,
                "output_index": output_index,
                "item": item,
            });
            self.push_event(out, "response.output_item.done", &payload);
        }
    }
}

impl Read for OpenAIResponsesBridgeSseReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let read = self.out_cursor.read(buf)?;
            if read > 0 {
                return Ok(read);
            }
            if self.finished {
                return Ok(0);
            }
            self.out_cursor = Cursor::new(self.next_chunk()?);
        }
    }
}
