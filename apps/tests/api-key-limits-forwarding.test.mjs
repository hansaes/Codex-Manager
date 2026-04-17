import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const appsRoot = path.resolve(import.meta.dirname, "..");

async function readAppFile(...segments) {
  return fs.readFile(path.join(appsRoot, ...segments), "utf8");
}

function assertIncludesAll(source, snippets, label) {
  for (const snippet of snippets) {
    assert.ok(
      source.includes(snippet),
      `${label} 缺少片段: ${snippet}`
    );
  }
}

test("account-client 创建/更新平台密钥时会透传限额字段", async () => {
  const source = await readAppFile("src", "lib", "api", "account-client.ts");

  assertIncludesAll(
    source,
    [
      'totalTokenLimit: params.totalTokenLimit ?? null',
      'totalCostUsdLimit: params.totalCostUsdLimit ?? null',
      'totalRequestLimit: params.totalRequestLimit ?? null',
    ],
    "apps/src/lib/api/account-client.ts"
  );
});

test("Tauri apikey 命令会接收并转发限额字段", async () => {
  const source = await readAppFile("src-tauri", "src", "commands", "apikey.rs");

  assertIncludesAll(
    source,
    [
      "total_token_limit: Option<i64>",
      "total_cost_usd_limit: Option<f64>",
      "total_request_limit: Option<i64>",
      '"totalTokenLimit": total_token_limit',
      '"totalCostUsdLimit": total_cost_usd_limit',
      '"totalRequestLimit": total_request_limit',
    ],
    "apps/src-tauri/src/commands/apikey.rs"
  );
});
