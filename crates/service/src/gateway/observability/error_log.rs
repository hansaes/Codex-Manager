use codexmanager_core::storage::{now_ts, GatewayErrorLog};

use crate::storage_helpers::open_storage;

#[derive(Debug, Clone, Default)]
pub(crate) struct GatewayErrorLogInput<'a> {
    pub(crate) trace_id: Option<&'a str>,
    pub(crate) key_id: Option<&'a str>,
    pub(crate) account_id: Option<&'a str>,
    pub(crate) request_path: &'a str,
    pub(crate) method: &'a str,
    pub(crate) stage: &'a str,
    pub(crate) error_kind: Option<&'a str>,
    pub(crate) upstream_url: Option<&'a str>,
    pub(crate) cf_ray: Option<&'a str>,
    pub(crate) status_code: Option<u16>,
    pub(crate) compression_enabled: bool,
    pub(crate) compression_retry_attempted: bool,
    pub(crate) message: &'a str,
}

/// 函数 `write_gateway_error_log`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-04
///
/// # 参数
/// - input: 参数 input
///
/// # 返回
/// 无
pub(crate) fn write_gateway_error_log(input: GatewayErrorLogInput<'_>) {
    let Some(storage) = open_storage() else {
        return;
    };
    let log = GatewayErrorLog {
        trace_id: input.trace_id.map(str::to_string),
        key_id: input.key_id.map(str::to_string),
        account_id: input.account_id.map(str::to_string),
        request_path: input.request_path.to_string(),
        method: input.method.to_string(),
        stage: input.stage.to_string(),
        error_kind: input.error_kind.map(str::to_string),
        upstream_url: input.upstream_url.map(str::to_string),
        cf_ray: input.cf_ray.map(str::to_string),
        status_code: input.status_code.map(i64::from),
        compression_enabled: input.compression_enabled,
        compression_retry_attempted: input.compression_retry_attempted,
        message: input.message.to_string(),
        created_at: now_ts(),
    };
    if let Err(err) = storage.insert_gateway_error_log(&log) {
        log::warn!("insert gateway error log failed: {err}");
    }
}
