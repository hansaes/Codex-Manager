import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { fetchWithRetry, runWithControl, RequestOptions } from "../utils/request";
import { DEFAULT_UNSUPPORTED_WEB_REASON } from "../runtime/runtime-capabilities";
import { useAppStore } from "../store/useAppStore";
import {
  getAppErrorMessage,
  isCommandMissingError,
  unwrapRpcPayload,
} from "./transport-errors";
export { getAppErrorMessage, isCommandMissingError } from "./transport-errors";
import { createWebCommandMap } from "./transport-web-commands";
import type { InvokeParams, WebCommandDescriptor } from "./transport-web-commands";
import { postJsonRpc } from "./rpc-http";
import {
  getCachedRuntimeCapabilities,
  isTauriRuntime,
  loadRuntimeCapabilities,
} from "./transport-runtime";
export {
  getCachedRuntimeCapabilities,
  isTauriRuntime,
  loadRuntimeCapabilities,
} from "./transport-runtime";

const WEB_COMMAND_MAP: Record<string, WebCommandDescriptor> =
  createWebCommandMap(postWebRpc);

async function invokeWebRpc<T>(
  method: string,
  params?: InvokeParams,
  options: RequestOptions = {},
): Promise<T> {
  const descriptor = WEB_COMMAND_MAP[method];
  if (!descriptor) {
    throw new Error("当前 Web / Docker 版暂不支持该操作");
  }
  if (descriptor.direct) {
    return (await descriptor.direct(params, options)) as T;
  }
  if (!descriptor.rpcMethod) {
    throw new Error("当前 Web / Docker 版暂不支持该操作");
  }
  return postWebRpc<T>(
    descriptor.rpcMethod,
    descriptor.mapParams ? descriptor.mapParams(params) : params ?? {},
    options,
  );
}

async function postWebRpc<T>(
  rpcMethod: string,
  params?: InvokeParams,
  options: RequestOptions = {},
): Promise<T> {
  const runtimeCapabilities = await loadRuntimeCapabilities();
  if (runtimeCapabilities.mode === "unsupported-web") {
    throw new Error(
      runtimeCapabilities.unsupportedReason || DEFAULT_UNSUPPORTED_WEB_REASON,
    );
  }

  return postJsonRpc<T>(
    fetchWithRetry,
    runtimeCapabilities.rpcBaseUrl,
    rpcMethod,
    params ?? {},
    options,
  );
}

export function withAddr(
  params: Record<string, unknown> = {},
): Record<string, unknown> {
  const addr = useAppStore.getState().serviceStatus.addr;
  return {
    addr: addr || null,
    ...params,
  };
}

export async function invokeFirst<T>(
  methods: string[],
  params?: Record<string, unknown>,
  options: RequestOptions = {},
): Promise<T> {
  let lastErr: unknown;
  for (const method of methods) {
    try {
      return await invoke<T>(method, params, options);
    } catch (err) {
      lastErr = err;
      if (!isCommandMissingError(err)) {
        throw err;
      }
    }
  }
  throw lastErr || new Error("未配置可用命令");
}

export async function invoke<T>(
  method: string,
  params?: InvokeParams,
  options: RequestOptions = {},
): Promise<T> {
  if (!isTauriRuntime()) {
    return invokeWebRpc(method, params, options);
  }

  const response = await runWithControl<unknown>(
    () => tauriInvoke(method, params || {}),
    options,
  );
  return unwrapRpcPayload<T>(response);
}

export async function requestlogListViaHttpRpc<T>(
  params: {
    query?: string;
    statusFilter?: string;
    page?: number;
    pageSize?: number;
  },
  addr: string,
  options: RequestOptions = {},
): Promise<T> {
  if (isTauriRuntime()) {
    return invoke<T>(
      "service_requestlog_list",
      {
        query: params.query || "",
        statusFilter: params.statusFilter || "all",
        page: params.page ?? 1,
        pageSize: params.pageSize ?? 20,
        addr,
      },
      options,
    );
  }

  return postJsonRpc<T>(
    fetchWithRetry,
    `http://${addr}/rpc`,
    "requestlog/list",
    {
      query: params.query || "",
      statusFilter: params.statusFilter || "all",
      page: params.page ?? 1,
      pageSize: params.pageSize ?? 20,
    },
    options,
  );
}
