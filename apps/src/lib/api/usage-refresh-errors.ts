import { getAppErrorMessage } from "./transport-errors";

export type UsageRefreshErrorTranslator = (
  message: string,
  values?: Record<string, string | number>,
) => string;

const REFRESH_TOKEN_401_MARKER = "refresh token failed with status 401";
const REFRESH_TOKEN_EXPIRED_MARKER = "refresh token has expired";
const REFRESH_TOKEN_REUSED_MARKER = "refresh token was already used";
const REFRESH_TOKEN_INVALIDATED_MARKER = "refresh token was revoked";

export function mapUsageRefreshErrorMessage(
  message: string,
  t: UsageRefreshErrorTranslator,
): string {
  const normalized = String(message || "").trim();
  const lowered = normalized.toLowerCase();

  if (!lowered.includes(REFRESH_TOKEN_401_MARKER)) {
    return normalized;
  }
  if (lowered.includes(REFRESH_TOKEN_EXPIRED_MARKER)) {
    return t("账号长期未登录，refresh 已过期，已改为不可用状态");
  }
  if (lowered.includes(REFRESH_TOKEN_REUSED_MARKER)) {
    return t("refresh token 已被使用，当前账号登录态失效，请重新登录");
  }
  if (lowered.includes(REFRESH_TOKEN_INVALIDATED_MARKER)) {
    return t("refresh token 已被吊销，当前账号登录态失效，请重新登录");
  }
  return t("账号登录态已失效，请重新登录");
}

export function formatUsageRefreshErrorMessage(
  error: unknown,
  t: UsageRefreshErrorTranslator,
): string {
  return mapUsageRefreshErrorMessage(getAppErrorMessage(error), t);
}
