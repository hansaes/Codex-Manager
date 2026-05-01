# Gateway Global Channel Priority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为网关增加账号 / 聚合 API 全局优先级开关，在主通道失败时自动切到次通道，并让 `/v1/models` 与对外错误返回和真实路由能力保持一致。

**Architecture:** 复用现有账号链与聚合链的内部重试逻辑，在 proxy 顶部加一层 family scheduler。local validation 负责同时准备账号和聚合两套请求变体，runtime config / app settings 暴露新的全局开关与顺序，`/v1/models` 在开关开启时改成并集视图。

**Tech Stack:** Rust service, tiny_http gateway, reqwest blocking client, Next.js App Router, TypeScript, Zustand, TanStack Query, Tailwind CSS.

---

### Task 1: 为 family 优先级与 failover 策略写 failing tests

**Files:**
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/upstream/proxy.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/upstream/protocol/aggregate_api.rs`

- [ ] **Step 1: 写 family 顺序与跨 family failover 判定 tests**

覆盖：
- `account_first`
- `aggregate_first`
- `401/403/429/5xx/timeout` 允许跨 family
- `400` 不跨 family

- [ ] **Step 2: 运行 Rust tests，确认先失败**

Run: `cargo test -p codexmanager-service gateway::upstream --lib`

Expected: FAIL，说明新行为尚未实现。

- [ ] **Step 3: 写最小 family scheduler / failover helpers**

- [ ] **Step 4: 再跑 Rust tests，确认变绿**

Run: `cargo test -p codexmanager-service gateway::upstream --lib`

Expected: PASS

### Task 2: 为全局优先级设置与 runtime config 写 failing tests

**Files:**
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/core/runtime_config.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/app_settings/api/current.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/app_settings/api/patch.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/app_settings/shared.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/app_settings/gateway.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/app_settings/runtime_sync.rs`

- [ ] **Step 1: 写设置默认值 / 规范化 / snapshot tests**

- [ ] **Step 2: 跑相关 tests 确认失败**

Run: `cargo test -p codexmanager-service app_settings --lib`

- [ ] **Step 3: 实现运行时配置与持久化同步**

- [ ] **Step 4: 再跑 tests 确认通过**

Run: `cargo test -p codexmanager-service app_settings --lib`

### Task 3: 为 local validation 双变体与 `/v1/models` 并集写 failing tests

**Files:**
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/local_validation/mod.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/local_validation/request.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/request/local_models.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/request/tests/local_models_tests.rs`

- [ ] **Step 1: 写 aggregate + account union model catalog tests**

覆盖：
- 开关关闭时保持旧行为
- 开关开启时返回并集
- 去重后 slug 正确

- [ ] **Step 2: 跑 local models tests，确认先失败**

Run: `cargo test -p codexmanager-service local_models --lib`

- [ ] **Step 3: 实现双请求变体与模型并集**

- [ ] **Step 4: 再跑 tests，确认通过**

Run: `cargo test -p codexmanager-service local_models --lib`

### Task 4: 为中文错误返回写 failing tests

**Files:**
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/mod.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/error_response.rs`
- Modify: `D:/code/Codex-Manager/crates/service/src/gateway/tests/error_response_tests.rs`

- [ ] **Step 1: 写 external client / raw client 错误返回 tests**

覆盖：
- bilingual message 对外返回中文
- raw client 保留英文
- 聚合 timeout / model not found 的中文摘要

- [ ] **Step 2: 跑 tests 确认失败**

Run: `cargo test -p codexmanager-service error_response --lib`

- [ ] **Step 3: 实现中文优先错误渲染**

- [ ] **Step 4: 再跑 tests 确认通过**

Run: `cargo test -p codexmanager-service error_response --lib`

### Task 5: 接入前端设置页

**Files:**
- Modify: `D:/code/Codex-Manager/apps/src/types/settings.ts`
- Modify: `D:/code/Codex-Manager/apps/src/lib/api/normalize.ts`
- Modify: `D:/code/Codex-Manager/apps/src/lib/store/useAppStore.ts`
- Modify: `D:/code/Codex-Manager/apps/src/app/settings/page.tsx`

- [ ] **Step 1: 扩展 settings 类型与 normalize 默认值**

- [ ] **Step 2: 在设置页新增全局优先级开关与顺序选择**

- [ ] **Step 3: 校正文案，让 `routeStrategy` 明确表示“通道内选路”**

- [ ] **Step 4: 跑前端构建**

Run: `pnpm run build:desktop`

Expected: PASS

### Task 6: 全量验证与 docker 验证

**Files:**
- Verify only

- [ ] **Step 1: 运行后端测试**

Run: `cargo test -p codexmanager-service`

- [ ] **Step 2: 运行前端构建**

Run: `pnpm run build:desktop`

- [ ] **Step 3: 用 docker 启动服务并验证全局优先级**

至少验证：
- 添加聚合 API
- 导入 `mimo-v2.5-pro`
- 开启全局优先级
- `account_first` 且账号不可用时外部请求能回退到聚合 API
- `/v1/models` 可见 `mimo-v2.5-pro`

- [ ] **Step 4: 确认没有覆盖已有未提交改动**

Run: `git status --short`
