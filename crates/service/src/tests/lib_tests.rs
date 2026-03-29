use super::*;
use codexmanager_core::rpc::types::{JsonRpcMessage, JsonRpcResponse};

fn response_result(resp: JsonRpcMessage) -> JsonRpcResponse {
    match resp {
        JsonRpcMessage::Response(resp) => resp,
        JsonRpcMessage::Error(err) => panic!("unexpected rpc error: {}", err.error.message),
        JsonRpcMessage::Notification(_) => panic!("unexpected rpc notification"),
        JsonRpcMessage::Request(_) => panic!("unexpected rpc request"),
    }
}

#[test]
fn login_complete_requires_params() {
    let req = JsonRpcRequest {
        id: 1.into(),
        method: "account/login/complete".to_string(),
        params: None,
        trace: None,
    };
    let resp = response_result(handle_request(req));
    let err = resp
        .result
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(err.contains("missing"));

    let req = JsonRpcRequest {
        id: 2.into(),
        method: "account/login/complete".to_string(),
        params: Some(serde_json::json!({ "code": "x" })),
        trace: None,
    };
    let resp = response_result(handle_request(req));
    let err = resp
        .result
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(err.contains("missing"));

    let req = JsonRpcRequest {
        id: 3.into(),
        method: "account/login/complete".to_string(),
        params: Some(serde_json::json!({ "state": "y" })),
        trace: None,
    };
    let resp = response_result(handle_request(req));
    let err = resp
        .result
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(err.contains("missing"));
}

#[test]
fn unknown_method_returns_jsonrpc_error() {
    let req = JsonRpcRequest {
        id: 9.into(),
        method: "not/a/method".to_string(),
        params: None,
        trace: None,
    };

    match handle_request(req) {
        JsonRpcMessage::Error(err) => {
            assert_eq!(err.id, 9.into());
            assert_eq!(err.error.code, -32601);
            assert_eq!(err.error.message, "unknown_method");
        }
        other => panic!("expected rpc error, got {other:?}"),
    }
}
