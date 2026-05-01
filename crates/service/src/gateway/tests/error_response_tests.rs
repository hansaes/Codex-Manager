use super::{terminal_text_response, with_trace_id_header};
use std::io::Read;
use tiny_http::Response;

#[test]
fn error_message_for_client_prefers_chinese_for_external_clients() {
    assert_eq!(
        crate::gateway::error_message_for_client(false, "无可用账号(no available account)"),
        "无可用账号"
    );
    assert_eq!(
        crate::gateway::error_message_for_client(
            false,
            "aggregate api model not found in selected catalog: mimo-v2.5-pro",
        ),
        "请求模型不在聚合 API 已选择目录中: mimo-v2.5-pro"
    );
}

#[test]
fn error_message_for_client_keeps_raw_message_for_internal_clients() {
    assert_eq!(
        crate::gateway::error_message_for_client(true, "无可用账号(no available account)"),
        "no available account"
    );
    assert_eq!(
        crate::gateway::error_message_for_client(
            true,
            "aggregate api model not found in selected catalog: mimo-v2.5-pro",
        ),
        "aggregate api model not found in selected catalog: mimo-v2.5-pro"
    );
}

/// 函数 `terminal_text_response_sets_error_code_header`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[test]
fn terminal_text_response_sets_error_code_header() {
    let response =
        terminal_text_response(503, "无可用账号(no available account)", Some("trc_test_1"));
    let content_type = response
        .headers()
        .iter()
        .find(|item| {
            item.field
                .as_str()
                .as_str()
                .eq_ignore_ascii_case("Content-Type")
        })
        .map(|item| item.value.as_str().to_string());
    let header = response
        .headers()
        .iter()
        .find(|item| {
            item.field
                .as_str()
                .as_str()
                .eq_ignore_ascii_case(crate::error_codes::ERROR_CODE_HEADER_NAME)
        })
        .map(|item| item.value.as_str().to_string());

    assert_eq!(
        content_type.as_deref(),
        Some("application/json; charset=utf-8")
    );
    assert_eq!(header.as_deref(), Some("no_available_account"));
    let trace_header = response
        .headers()
        .iter()
        .find(|item| {
            item.field
                .as_str()
                .as_str()
                .eq_ignore_ascii_case(crate::error_codes::TRACE_ID_HEADER_NAME)
        })
        .map(|item| item.value.as_str().to_string());
    assert_eq!(trace_header.as_deref(), Some("trc_test_1"));

    let mut body = String::new();
    response
        .into_reader()
        .read_to_string(&mut body)
        .expect("read response body");
    assert!(
        body.contains("\"message\":\"无可用账号\""),
        "unexpected response body: {body}"
    );
    assert!(
        !body.contains("no available account"),
        "response should return chinese message for external clients: {body}"
    );
}

/// 函数 `with_trace_id_header_appends_trace_header`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[test]
fn with_trace_id_header_appends_trace_header() {
    let response = with_trace_id_header(Response::from_string("ok"), Some("trc_ok_1"));
    let trace_header = response
        .headers()
        .iter()
        .find(|item| {
            item.field
                .as_str()
                .as_str()
                .eq_ignore_ascii_case(crate::error_codes::TRACE_ID_HEADER_NAME)
        })
        .map(|item| item.value.as_str().to_string());
    assert_eq!(trace_header.as_deref(), Some("trc_ok_1"));
}
