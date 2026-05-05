import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";
import ts from "../node_modules/typescript/lib/typescript.js";

const appsRoot = path.resolve(import.meta.dirname, "..");
const sourcePath = path.join(
  appsRoot,
  "src",
  "lib",
  "api",
  "usage-refresh-errors.ts"
);
const transportErrorsSourcePath = path.join(
  appsRoot,
  "src",
  "lib",
  "api",
  "transport-errors.ts"
);

async function loadUsageRefreshErrorsModule() {
  const [source, transportErrorsSource] = await Promise.all([
    fs.readFile(sourcePath, "utf8"),
    fs.readFile(transportErrorsSourcePath, "utf8"),
  ]);

  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: sourcePath,
  });
  const transportErrorsCompiled = ts.transpileModule(transportErrorsSource, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: transportErrorsSourcePath,
  });

  const tempDir = await fs.mkdtemp(
    path.join(os.tmpdir(), "codexmanager-usage-refresh-errors-")
  );
  const tempFile = path.join(tempDir, "usage-refresh-errors.mjs");
  const transportErrorsFile = path.join(tempDir, "transport-errors.mjs");

  await fs.writeFile(
    tempFile,
    compiled.outputText.replace(
      /from "\.\/transport-errors"/g,
      'from "./transport-errors.mjs"'
    ),
    "utf8"
  );
  await fs.writeFile(transportErrorsFile, transportErrorsCompiled.outputText, "utf8");
  return import(pathToFileURL(tempFile).href);
}

const usageRefreshErrors = await loadUsageRefreshErrorsModule();
const passthroughTranslate = (message) => message;

test("formatUsageRefreshErrorMessage 仅把 expired 401 映射为 refresh 过期", () => {
  assert.equal(
    usageRefreshErrors.formatUsageRefreshErrorMessage(
      new Error(
        "refresh token failed with status 401 Unauthorized: Your access token could not be refreshed because your refresh token has expired. Please log out and sign in again."
      ),
      passthroughTranslate
    ),
    "账号长期未登录，refresh 已过期，已改为不可用状态"
  );
});

test("formatUsageRefreshErrorMessage 会把 reused 401 映射为重新登录提示", () => {
  assert.equal(
    usageRefreshErrors.formatUsageRefreshErrorMessage(
      new Error(
        "refresh token failed with status 401 Unauthorized: Your access token could not be refreshed because your refresh token was already used. Please log out and sign in again."
      ),
      passthroughTranslate
    ),
    "refresh token 已被使用，当前账号登录态失效，请重新登录"
  );
});

test("formatUsageRefreshErrorMessage 会把 revoked 401 映射为更准确的失效提示", () => {
  assert.equal(
    usageRefreshErrors.formatUsageRefreshErrorMessage(
      new Error(
        "refresh token failed with status 401 Unauthorized: Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again."
      ),
      passthroughTranslate
    ),
    "refresh token 已被吊销，当前账号登录态失效，请重新登录"
  );
});

test("formatUsageRefreshErrorMessage 会把未知 refresh 401 映射为通用重新登录提示", () => {
  assert.equal(
    usageRefreshErrors.formatUsageRefreshErrorMessage(
      new Error(
        "refresh token failed with status 401 Unauthorized: Your access token could not be refreshed. Please log out and sign in again."
      ),
      passthroughTranslate
    ),
    "账号登录态已失效，请重新登录"
  );
});

test("mapUsageRefreshErrorMessage 不会误改写非 refresh 401 或其他错误", () => {
  const nonRefresh401 = "usage endpoint failed: status=401 Unauthorized body=forbidden";
  const otherError = "network timeout";

  assert.equal(
    usageRefreshErrors.mapUsageRefreshErrorMessage(nonRefresh401, passthroughTranslate),
    nonRefresh401
  );
  assert.equal(
    usageRefreshErrors.mapUsageRefreshErrorMessage(otherError, passthroughTranslate),
    otherError
  );
});
