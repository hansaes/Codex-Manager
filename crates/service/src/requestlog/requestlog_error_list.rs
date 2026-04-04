use codexmanager_core::rpc::types::{GatewayErrorLogListResult, GatewayErrorLogSummary};
use codexmanager_core::storage::GatewayErrorLog;

use crate::storage_helpers::open_storage;

const DEFAULT_GATEWAY_ERROR_LOG_LIMIT: i64 = 50;

/// 函数 `read_gateway_error_logs`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-04
///
/// # 参数
/// - limit: 参数 limit
///
/// # 返回
/// 返回函数执行结果
pub(crate) fn read_gateway_error_logs(
    limit: Option<i64>,
) -> Result<GatewayErrorLogListResult, String> {
    let storage = open_storage().ok_or_else(|| "open storage failed".to_string())?;
    let items = storage
        .list_gateway_error_logs(limit.unwrap_or(DEFAULT_GATEWAY_ERROR_LOG_LIMIT))
        .map_err(|err| format!("list gateway error logs failed: {err}"))?;
    Ok(GatewayErrorLogListResult {
        items: items.into_iter().map(to_gateway_error_log_summary).collect(),
    })
}

fn to_gateway_error_log_summary(item: GatewayErrorLog) -> GatewayErrorLogSummary {
    GatewayErrorLogSummary {
        trace_id: item.trace_id,
        key_id: item.key_id,
        account_id: item.account_id,
        request_path: item.request_path,
        method: item.method,
        stage: item.stage,
        error_kind: item.error_kind,
        upstream_url: item.upstream_url,
        cf_ray: item.cf_ray,
        status_code: item.status_code,
        compression_enabled: item.compression_enabled,
        compression_retry_attempted: item.compression_retry_attempted,
        message: item.message,
        created_at: item.created_at,
    }
}
