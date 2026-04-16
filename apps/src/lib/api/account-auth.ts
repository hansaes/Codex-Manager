import type {
  ChatgptAuthTokensRefreshResult,
  CurrentAccessTokenAccount,
  CurrentAccessTokenAccountReadResult,
  LoginStatusResult,
} from "../../types";

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function readStringField(payload: unknown, key: string, fallback = ""): string {
  const source = asRecord(payload);
  const value = source?.[key];
  return typeof value === "string" ? value.trim() : fallback;
}

function readBooleanField(payload: unknown, key: string, fallback = false): boolean {
  const source = asRecord(payload);
  const value = source?.[key];
  return typeof value === "boolean" ? value : fallback;
}

function readNullableStringField(payload: unknown, key: string): string | null {
  const value = readStringField(payload, key);
  return value ? value : null;
}

export function readLoginStatusResult(payload: unknown): LoginStatusResult {
  return {
    status: readStringField(payload, "status"),
    error: readStringField(payload, "error"),
  };
}

export function readCurrentAccessTokenAccount(
  payload: unknown
): CurrentAccessTokenAccount | null {
  const source = asRecord(payload);
  if (!source) {
    return null;
  }

  return {
    type: readStringField(source, "type"),
    accountId: readStringField(source, "accountId"),
    email: readStringField(source, "email"),
    planType: readStringField(source, "planType"),
    planTypeRaw: readNullableStringField(source, "planTypeRaw"),
    chatgptAccountId: readNullableStringField(source, "chatgptAccountId"),
    workspaceId: readNullableStringField(source, "workspaceId"),
    status: readStringField(source, "status"),
  };
}

export function readCurrentAccessTokenAccountReadResult(
  payload: unknown
): CurrentAccessTokenAccountReadResult {
  const source = asRecord(payload);
  return {
    account: readCurrentAccessTokenAccount(source?.account),
    requiresOpenaiAuth: readBooleanField(payload, "requiresOpenaiAuth"),
  };
}

export function readChatgptAuthTokensRefreshResult(
  payload: unknown
): ChatgptAuthTokensRefreshResult {
  return {
    accessToken: readStringField(payload, "accessToken"),
    chatgptAccountId: readStringField(payload, "chatgptAccountId"),
    chatgptPlanType: readNullableStringField(payload, "chatgptPlanType"),
  };
}
