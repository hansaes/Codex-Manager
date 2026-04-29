use codexmanager_core::storage::{ApiKeyQuotaLimits, ApiKeyTokenUsageSummary};

fn normalize_count_limit(limit: Option<i64>, label: &str) -> Result<Option<i64>, String> {
    match limit {
        None => Ok(None),
        Some(value) if value <= 0 => Ok(None),
        Some(value) => {
            if value > i32::MAX as i64 * 1_000_000 {
                return Err(format!("{label} is too large"));
            }
            Ok(Some(value))
        }
    }
}

pub(crate) fn normalize_total_token_limit(limit: Option<i64>) -> Result<Option<i64>, String> {
    normalize_count_limit(limit, "totalTokenLimit")
}

pub(crate) fn normalize_total_request_limit(limit: Option<i64>) -> Result<Option<i64>, String> {
    normalize_count_limit(limit, "totalRequestLimit")
}

pub(crate) fn normalize_total_cost_usd_limit(limit: Option<f64>) -> Result<Option<f64>, String> {
    match limit {
        None => Ok(None),
        Some(value) if !value.is_finite() => Err("totalCostUsdLimit must be finite".to_string()),
        Some(value) if value <= 0.0 => Ok(None),
        Some(value) => Ok(Some(value)),
    }
}

pub(crate) fn build_quota_exceeded_message(
    key_name: Option<&str>,
    key_id: &str,
    limits: &ApiKeyQuotaLimits,
    usage: &ApiKeyTokenUsageSummary,
) -> Option<String> {
    let mut exceeded = Vec::new();

    if let Some(limit) = limits
        .total_request_limit
        .filter(|limit| usage.request_count >= *limit)
    {
        exceeded.push(format!("requests {}/{}", usage.request_count, limit));
    }
    if let Some(limit) = limits
        .total_token_limit
        .filter(|limit| usage.total_tokens >= *limit)
    {
        exceeded.push(format!("tokens {}/{}", usage.total_tokens, limit));
    }
    if let Some(limit) = limits
        .total_cost_usd_limit
        .filter(|limit| usage.estimated_cost_usd >= *limit)
    {
        exceeded.push(format!(
            "cost ${:.4}/${:.4}",
            usage.estimated_cost_usd, limit
        ));
    }

    if exceeded.is_empty() {
        return None;
    }

    let display_name = key_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(key_id);
    Some(format!(
        "api key quota exceeded: {} [{}], {}",
        display_name,
        key_id,
        exceeded.join(", ")
    ))
}
