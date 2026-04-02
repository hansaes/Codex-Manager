use codexmanager_core::storage::Storage;
use reqwest::header::HeaderValue;

pub(in super::super) enum UpstreamOutcomeDecision {
    Failover,
    RespondUpstream,
}

/// 函数 `decide_upstream_outcome`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - in super: 参数 in super
///
/// # 返回
/// 返回函数执行结果
pub(in super::super) fn decide_upstream_outcome<F>(
    storage: &Storage,
    account_id: &str,
    status: reqwest::StatusCode,
    upstream_content_type: Option<&HeaderValue>,
    url: &str,
    has_more_candidates: bool,
    mut log_gateway_result: F,
) -> UpstreamOutcomeDecision
where
    F: FnMut(Option<&str>, u16, Option<&str>),
{
    let is_official_target = super::super::config::is_official_openai_target(url);
    if status.is_success() {
        super::super::super::clear_account_cooldown(account_id);
        log_gateway_result(Some(url), status.as_u16(), None);
        return UpstreamOutcomeDecision::RespondUpstream;
    }

    let is_challenge =
        super::super::super::is_upstream_challenge_response(status.as_u16(), upstream_content_type);
    if is_challenge {
        super::super::super::mark_account_cooldown(
            account_id,
            super::super::super::CooldownReason::Challenge,
        );
        log_gateway_result(
            Some(url),
            status.as_u16(),
            Some("upstream challenge blocked"),
        );
        if has_more_candidates {
            return UpstreamOutcomeDecision::Failover;
        }
        return UpstreamOutcomeDecision::RespondUpstream;
    }

    if is_official_target && status.as_u16() == 401 {
        log_gateway_result(Some(url), status.as_u16(), Some("upstream unauthorized"));
        return UpstreamOutcomeDecision::RespondUpstream;
    }

    if !is_official_target {
        if matches!(status.as_u16(), 429 | 500..=599) {
            // 中文注释：自定义上游继续保留原有容错策略，避免破坏兼容行为。
            super::super::super::mark_account_cooldown_for_status(account_id, status.as_u16());
        }
        if status.as_u16() == 404 && has_more_candidates {
            super::super::super::mark_account_cooldown_for_status(account_id, status.as_u16());
            log_gateway_result(
                Some(url),
                status.as_u16(),
                Some("upstream not-found failover"),
            );
            return UpstreamOutcomeDecision::Failover;
        }
        if status.as_u16() == 429 {
            log_gateway_result(Some(url), status.as_u16(), Some("upstream rate-limited"));
            if has_more_candidates {
                return UpstreamOutcomeDecision::Failover;
            }
            return UpstreamOutcomeDecision::RespondUpstream;
        }
        if matches!(status.as_u16(), 500..=599) {
            log_gateway_result(Some(url), status.as_u16(), Some("upstream server error"));
            if has_more_candidates {
                return UpstreamOutcomeDecision::Failover;
            }
            return UpstreamOutcomeDecision::RespondUpstream;
        }
    }

    let _ = crate::usage_refresh::enqueue_usage_refresh_for_account(account_id);
    let should_failover = (!is_official_target || status.as_u16() != 401)
        && super::super::super::should_failover_from_cached_snapshot(storage, account_id);
    if should_failover {
        if is_official_target {
            super::super::super::mark_account_cooldown(
                account_id,
                super::super::super::CooldownReason::Default,
            );
            log_gateway_result(Some(url), status.as_u16(), Some("upstream account exhausted"));
        } else {
            super::super::super::mark_account_cooldown_for_status(account_id, status.as_u16());
            log_gateway_result(Some(url), status.as_u16(), Some("upstream non-success"));
        }
        if has_more_candidates {
            return UpstreamOutcomeDecision::Failover;
        }
        return UpstreamOutcomeDecision::RespondUpstream;
    }

    log_gateway_result(Some(url), status.as_u16(), Some("upstream non-success"));
    UpstreamOutcomeDecision::RespondUpstream
}

#[cfg(test)]
#[path = "../tests/support/outcome_tests.rs"]
mod tests;
