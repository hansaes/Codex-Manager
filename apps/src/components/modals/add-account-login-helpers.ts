import type { LoginStartResult } from "../../types/auth";

export const LOGIN_POLL_TIMEOUT_MS = 5 * 60 * 1000;
export const LOGIN_POLL_INTERVAL_MS = 1500;

export interface PendingLoginWindow {
  close?: () => void;
  closed?: boolean;
  location?: {
    href?: string;
    replace?: (url: string) => void;
  };
  opener?: unknown;
}

function readNonEmptyString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function resolveLoginLaunchUrl(
  result?: Pick<LoginStartResult, "authUrl" | "verificationUrl"> | null
): string {
  if (!result) {
    return "";
  }
  return (
    readNonEmptyString(result.authUrl) ||
    readNonEmptyString(result.verificationUrl)
  );
}

export function openPendingLoginWindow(): PendingLoginWindow | null {
  if (typeof window === "undefined" || typeof window.open !== "function") {
    return null;
  }
  return window.open("", "_blank");
}

export function navigatePendingLoginWindow(
  pendingWindow: PendingLoginWindow | null | undefined,
  url: string
): boolean {
  const normalizedUrl = readNonEmptyString(url);
  if (!pendingWindow || !normalizedUrl) {
    return false;
  }

  try {
    pendingWindow.opener = null;
  } catch {
    // ignore
  }

  const location = pendingWindow.location;
  if (!location) {
    return false;
  }

  if (typeof location.replace === "function") {
    location.replace(normalizedUrl);
    return true;
  }

  location.href = normalizedUrl;
  return true;
}

export function closePendingLoginWindow(
  pendingWindow: PendingLoginWindow | null | undefined
): void {
  if (!pendingWindow || pendingWindow.closed || typeof pendingWindow.close !== "function") {
    return;
  }
  try {
    pendingWindow.close();
  } catch {
    // ignore
  }
}
