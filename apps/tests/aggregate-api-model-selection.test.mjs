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
    assert.ok(source.includes(snippet), `${label} 缺少片段: ${snippet}`);
  }
}

test("account-client 暴露聚合模型预览与保存方法", async () => {
  const source = await readAppFile("src", "lib", "api", "account-client.ts");

  assertIncludesAll(
    source,
    [
      "previewAggregateApiModels",
      "saveAggregateApiModels",
      "testAggregateApiModel",
      "\"service_aggregate_api_preview_models\"",
      "\"service_aggregate_api_save_models\"",
      "\"service_aggregate_api_test_model\"",
    ],
    "apps/src/lib/api/account-client.ts"
  );
});

test("聚合 API 页面接入模型选择弹窗且默认不自动全选", async () => {
  const source = await readAppFile("src", "app", "aggregate-api", "page.tsx");

  assertIncludesAll(
    source,
    [
      "AggregateApiModelPickerModal",
      "previewAggregateApiModels",
      "saveAggregateApiModels",
      "selectedModelSlugs",
      "listAggregateApiModels(api.id)",
      "openModelsMutation",
      "查看模型",
    ],
    "apps/src/app/aggregate-api/page.tsx"
  );
});

test("模型选择弹窗为列表区域提供可收缩滚动容器", async () => {
  const source = await readAppFile(
    "src",
    "components",
    "modals",
    "aggregate-api-model-picker-modal.tsx"
  );

  assertIncludesAll(
    source,
    [
      "max-h-[90vh]",
      "overflow-hidden",
      "min-h-0",
      "flex-1 overflow-y-auto",
      "overscroll-contain",
    ],
    "apps/src/components/modals/aggregate-api-model-picker-modal.tsx"
  );
});

test("模型选择弹窗展示已导入状态并提示取消勾选可移除", async () => {
  const source = await readAppFile(
    "src",
    "components",
    "modals",
    "aggregate-api-model-picker-modal.tsx"
  );

  assertIncludesAll(
    source,
    [
      "alreadyImportedModelSlugs",
      "已导入",
      "取消勾选后，保存时会从当前聚合 API 中移除",
      "onSyncUpstream",
      "同步上游",
    ],
    "apps/src/components/modals/aggregate-api-model-picker-modal.tsx"
  );
});

test("模型选择弹窗支持手动添加与测试单个模型", async () => {
  const source = await readAppFile(
    "src",
    "components",
    "modals",
    "aggregate-api-model-picker-modal.tsx"
  );

  assertIncludesAll(
    source,
    [
      "onAddManualModel",
      "onTestModel",
      "手动添加模型",
      "模型 ID，例如 gpt-5.4",
      "测试中...",
    ],
    "apps/src/components/modals/aggregate-api-model-picker-modal.tsx"
  );
});

test("首次同步失败时仍会打开模型弹窗并允许手动添加", async () => {
  const source = await readAppFile("src", "app", "aggregate-api", "page.tsx");

  assertIncludesAll(
    source,
    [
      'mode: "manual"',
      "openModelPicker(api, [], new Set())",
      "同步上游失败，可先手动添加模型",
    ],
    "apps/src/app/aggregate-api/page.tsx"
  );
});

test("保存聚合 API 模型后会刷新共享模型目录与启动快照缓存", async () => {
  const source = await readAppFile("src", "app", "aggregate-api", "page.tsx");

  assertIncludesAll(
    source,
    [
      'invalidateQueries({ queryKey: ["apikey-models"] })',
      'invalidateQueries({ queryKey: ["managed-model-catalog"] })',
      'invalidateQueries({ queryKey: ["startup-snapshot"] })',
    ],
    "apps/src/app/aggregate-api/page.tsx"
  );
});

test("平台密钥聚合 API 轮转不再绑定单个聚合 API", async () => {
  const source = await readAppFile(
    "src",
    "components",
    "modals",
    "api-key-modal.tsx"
  );

  assertIncludesAll(
    source,
    [
      "aggregateApiId: null",
      "无需把平台密钥绑定到某一个聚合 API",
      "listAggregateApiModels(api.id)",
    ],
    "apps/src/components/modals/api-key-modal.tsx"
  );
  assert.ok(
    !source.includes("throw new Error(t(\"请选择聚合 API\"))"),
    "平台密钥保存时不应要求选择聚合 API"
  );
});
