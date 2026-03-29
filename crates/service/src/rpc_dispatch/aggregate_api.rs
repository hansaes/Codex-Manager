use codexmanager_core::rpc::types::{
    AggregateApiListResult, JsonRpcRequest, JsonRpcResponse,
};

use crate::{
    create_aggregate_api, delete_aggregate_api, list_aggregate_apis, read_aggregate_api_secret,
    test_aggregate_api_connection, update_aggregate_api,
};

fn api_id_param(req: &JsonRpcRequest) -> Option<&str> {
    super::str_param(req, "id").or_else(|| super::str_param(req, "apiId"))
}

pub(super) fn try_handle(req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let result = match req.method.as_str() {
        "aggregateApi/list" => super::value_or_error(
            list_aggregate_apis().map(|items| AggregateApiListResult { items }),
        ),
        "aggregateApi/create" => {
            let provider_type = super::string_param(req, "providerType");
            let supplier_name = super::string_param(req, "supplierName");
            let sort = super::i64_param(req, "sort");
            let url = super::string_param(req, "url");
            let key = super::string_param(req, "key");
            super::value_or_error(create_aggregate_api(
                url,
                key,
                provider_type,
                supplier_name,
                sort,
            ))
        }
        "aggregateApi/update" => {
            let api_id = api_id_param(req).unwrap_or("");
            let provider_type = super::string_param(req, "providerType");
            let supplier_name = super::string_param(req, "supplierName");
            let sort = super::i64_param(req, "sort");
            let url = super::string_param(req, "url");
            let key = super::string_param(req, "key");
            super::ok_or_error(update_aggregate_api(
                api_id,
                url,
                key,
                provider_type,
                supplier_name,
                sort,
            ))
        }
        "aggregateApi/readSecret" => {
            let api_id = api_id_param(req).unwrap_or("");
            super::value_or_error(read_aggregate_api_secret(api_id))
        }
        "aggregateApi/delete" => {
            let api_id = api_id_param(req).unwrap_or("");
            super::ok_or_error(delete_aggregate_api(api_id))
        }
        "aggregateApi/testConnection" => {
            let api_id = api_id_param(req).unwrap_or("");
            super::value_or_error(test_aggregate_api_connection(api_id))
        }
        _ => return None,
    };

    Some(super::response(req, result))
}

#[cfg(test)]
mod tests {
    use super::try_handle;
    use codexmanager_core::rpc::types::JsonRpcRequest;

    fn rpc_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            id: 1.into(),
            method: method.to_string(),
            params: Some(params),
            trace: None,
        }
    }

    fn error_message(resp: &codexmanager_core::rpc::types::JsonRpcResponse) -> String {
        resp.result
            .get("error")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn aggregate_api_update_accepts_id_and_api_id() {
        let missing = try_handle(&rpc_request(
            "aggregateApi/update",
            serde_json::json!({ "supplierName": "codex" }),
        ))
        .expect("response");
        assert_eq!(error_message(&missing), "aggregate api id required");

        let with_id = try_handle(&rpc_request(
            "aggregateApi/update",
            serde_json::json!({ "id": "ag_test", "supplierName": "codex" }),
        ))
        .expect("response");
        assert_ne!(error_message(&with_id), "aggregate api id required");

        let with_api_id = try_handle(&rpc_request(
            "aggregateApi/update",
            serde_json::json!({ "apiId": "ag_test", "supplierName": "codex" }),
        ))
        .expect("response");
        assert_ne!(error_message(&with_api_id), "aggregate api id required");
    }

    #[test]
    fn aggregate_api_test_connection_accepts_id_and_api_id() {
        let missing = try_handle(&rpc_request("aggregateApi/testConnection", serde_json::json!({})))
            .expect("response");
        assert_eq!(error_message(&missing), "aggregate api id required");

        let with_id = try_handle(&rpc_request(
            "aggregateApi/testConnection",
            serde_json::json!({ "id": "ag_test" }),
        ))
        .expect("response");
        assert_ne!(error_message(&with_id), "aggregate api id required");

        let with_api_id = try_handle(&rpc_request(
            "aggregateApi/testConnection",
            serde_json::json!({ "apiId": "ag_test" }),
        ))
        .expect("response");
        assert_ne!(error_message(&with_api_id), "aggregate api id required");
    }
}
