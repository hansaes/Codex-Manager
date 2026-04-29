use codexmanager_core::storage::{ApiKey, Storage};

use crate::storage_helpers::{hash_platform_key, open_storage, StorageHandle};

/// 函数 `open_storage_or_error`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) fn open_storage_or_error() -> Result<StorageHandle, super::LocalValidationError> {
    open_storage().ok_or_else(|| {
        super::LocalValidationError::new(
            500,
            crate::gateway::bilingual_error("存储不可用", "storage unavailable"),
        )
    })
}

/// 函数 `load_active_api_key`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) fn load_active_api_key(
    storage: &Storage,
    platform_key: &str,
    request_url: &str,
    debug: bool,
) -> Result<ApiKey, super::LocalValidationError> {
    let key_hash = hash_platform_key(platform_key);
    let api_key = storage.find_api_key_by_hash(&key_hash).map_err(|err| {
        super::LocalValidationError::new(
            500,
            crate::gateway::bilingual_error("读取存储失败", format!("storage read failed: {err}")),
        )
    })?;

    let Some(api_key) = api_key else {
        if debug {
            log::warn!(
                "event=gateway_auth_invalid path={} status=403 key_hash_prefix={}",
                request_url,
                &key_hash[..8]
            );
        }
        return Err(super::LocalValidationError::new(
            403,
            crate::gateway::MISSING_AUTH_JSON_OPENAI_API_KEY_ERROR,
        ));
    };

    if api_key.status != "active" {
        if debug {
            log::warn!(
                "event=gateway_auth_disabled path={} status=403 key_id={}",
                request_url,
                api_key.id
            );
        }
        return Err(super::LocalValidationError::new(
            403,
            crate::gateway::bilingual_error("API Key 已禁用", "api key disabled"),
        ));
    }

    if let Some(limits) = storage
        .find_api_key_quota_limits(&api_key.id)
        .map_err(|err| {
            super::LocalValidationError::new(
                500,
                crate::gateway::bilingual_error(
                    "读取平台密钥额度限制失败",
                    format!("api key quota limit read failed: {err}"),
                ),
            )
        })?
    {
        let usage = storage
            .summarize_request_token_stats_for_key(&api_key.id)
            .map_err(|err| {
                super::LocalValidationError::new(
                    500,
                    crate::gateway::bilingual_error(
                        "读取平台密钥使用统计失败",
                        format!("api key usage summary read failed: {err}"),
                    ),
                )
            })?;
        if let Some(message) = crate::apikey::quota_limits::build_quota_exceeded_message(
            api_key.name.as_deref(),
            api_key.id.as_str(),
            &limits,
            &usage,
        ) {
            if debug {
                log::warn!(
                    "event=gateway_auth_quota_exceeded path={} status=429 key_id={} message={}",
                    request_url,
                    api_key.id,
                    message
                );
            }
            return Err(super::LocalValidationError::new(
                429,
                crate::gateway::bilingual_error("平台密钥额度已达上限", message),
            ));
        }
    }

    Ok(api_key)
}
