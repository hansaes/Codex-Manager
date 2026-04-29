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
  "components",
  "modals",
  "add-account-login-helpers.ts"
);

async function loadHelpersModule() {
  const source = await fs.readFile(sourcePath, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: sourcePath,
  });

  const tempDir = await fs.mkdtemp(
    path.join(os.tmpdir(), "codexmanager-add-account-login-helpers-")
  );
  const tempFile = path.join(tempDir, "add-account-login-helpers.mjs");
  await fs.writeFile(tempFile, compiled.outputText, "utf8");
  return import(pathToFileURL(tempFile).href);
}

const helpers = await loadHelpersModule();

test("登录轮询超时时间对齐 CLIProxyAPI 的 5 分钟等待窗口", () => {
  assert.equal(helpers.LOGIN_POLL_TIMEOUT_MS, 5 * 60 * 1000);
});

test("登录链接优先使用 authUrl，其次回退 verificationUrl", () => {
  assert.equal(
    helpers.resolveLoginLaunchUrl({
      authUrl: "https://auth.example.com",
      verificationUrl: "https://verify.example.com",
    }),
    "https://auth.example.com"
  );
  assert.equal(
    helpers.resolveLoginLaunchUrl({
      authUrl: "",
      verificationUrl: "https://verify.example.com",
    }),
    "https://verify.example.com"
  );
  assert.equal(helpers.resolveLoginLaunchUrl(null), "");
});

test("openPendingLoginWindow 在浏览器环境下同步预开标签页", () => {
  const opened = [];
  globalThis.window = {
    open: (...args) => {
      opened.push(args);
      return { location: { replace() {} } };
    },
  };

  const pendingWindow = helpers.openPendingLoginWindow();
  assert.deepEqual(opened, [["", "_blank"]]);
  assert.ok(pendingWindow);

  delete globalThis.window;
});

test("navigatePendingLoginWindow 使用 replace 导航到授权页", () => {
  const replaced = [];
  const pendingWindow = {
    location: {
      replace: (url) => replaced.push(url),
    },
    opener: { foo: "bar" },
  };

  assert.equal(
    helpers.navigatePendingLoginWindow(
      pendingWindow,
      "https://auth.example.com/oauth"
    ),
    true
  );
  assert.deepEqual(replaced, ["https://auth.example.com/oauth"]);
  assert.equal(pendingWindow.opener, null);
});
