# Gateway Global Channel Priority Design

**Date:** 2026-05-01  
**Scope:** 为网关新增“账号 / 聚合 API 全局优先级”开关、跨通道 failover、模型列表合并和中文错误返回。

## 1. Goal

让外部请求不再被 `rotationStrategy` 限定在单一路由家族内，而是可以在“账号”和“聚合 API”两类上游之间按全局顺序切换：

1. 支持全局开关启用统一优先级链。
2. 支持 `account_first` 和 `aggregate_first` 两种顺序。
3. 当主通道出现无候选、模型不支持、`401/403/429/5xx/超时` 等失败时自动切到次通道。
4. `GET /v1/models` 返回账号模型缓存与聚合模型目录的并集，避免“能调不能看见”。
5. 外部返回尽量优先给中文业务错误，同时对 Codex 等内部客户端保留 raw error 兼容。

## 2. Current Problems

### 2.1 账号链与聚合链互斥

当前 [proxy.rs](D:/code/Codex-Manager/crates/service/src/gateway/upstream/proxy.rs) 里只会二选一：

- `account_rotation` 走账号候选链
- `aggregate_api_rotation` 走聚合 API 候选链

两个 family 之间没有统一调度层。

### 2.2 聚合 API 内部可重试，但不会回退到账号链

[aggregate_api.rs](D:/code/Codex-Manager/crates/service/src/gateway/upstream/protocol/aggregate_api.rs) 已经支持多个聚合 API 候选和部分重试，但整个聚合 family 失败后会直接对外报错，不会切到账号 family。

### 2.3 `/v1/models` 与真实可路由能力可能不一致

[local_models.rs](D:/code/Codex-Manager/crates/service/src/gateway/request/local_models.rs) 现在只会：

- 聚合 key 返回聚合目录
- 非聚合 key 返回账号模型缓存

如果全局优先级开启，次通道能力不会体现在模型列表里。

### 2.4 对外错误默认偏原始英文

[mod.rs](D:/code/Codex-Manager/crates/service/src/gateway/mod.rs) 中 `error_message_for_client` 当前基本总是提取英文 raw tail，外部调用时中文可读性不够。

## 3. Approaches

### Approach A — 统一候选池

把账号和聚合 API 都抽象成同一类候选，统一排序、轮转、失败切换。

优点：模型最统一。  
缺点：侵入太大，现有账号 cooldown、聚合模型过滤、协议透传路径都要重写。

### Approach B — 推荐：family chain

保留“账号链”和“聚合链”各自内部机制，在其外层新增一个统一 family 调度层：

- 先跑主 family
- family 内部自行重试 / failover
- family 全部失败后，再按错误类型决定是否切到次 family

优点：复用现有稳定逻辑最多，改动边界清晰。  
缺点：需要在 local validation 阶段同时准备两套请求变体。

### Approach C — 只给现有 strategy 补 fallback family

保留 `rotationStrategy` 主导入口，只额外配置一个兜底 family。

优点：改动最小。  
缺点：不是全局优先级，设置心智负担仍高。

**Decision:** 采用 **Approach B**。

## 4. Architecture

### 4.1 Runtime config

新增两个全局设置：

- `gateway.global_channel_priority_enabled`
- `gateway.global_channel_priority_order`

运行时暴露：

- `global_channel_priority_enabled() -> bool`
- `current_global_channel_priority_order() -> "account_first" | "aggregate_first"`
- `set_global_channel_priority_enabled(bool)`
- `set_global_channel_priority_order(&str)`

开关关闭时，保持 `rotationStrategy` 旧行为不变。  
开关开启时，由全局顺序决定主 family。

### 4.2 Local validation prepares both variants

当前 local validation 会根据 `rotationStrategy` 只生成一套请求变体：

- 账号 family：协议适配、模型覆写、会话绑定
- 聚合 family：原路径透传、聚合目录校验

启用全局优先级时，需要额外生成“另一套 family 变体”，供 proxy 在跨 family failover 时使用。  
这样可以避免在 proxy 阶段重新读取请求体或重复解析完整请求。

### 4.3 Proxy family scheduler

在 [proxy.rs](D:/code/Codex-Manager/crates/service/src/gateway/upstream/proxy.rs) 顶部增加 family 调度：

1. 按全局顺序生成执行序列。
2. 执行主 family。
3. 如果主 family 成功，直接返回。
4. 如果主 family 失败且属于“可跨 family 切换失败”，执行次 family。
5. 如果两边都失败，对外返回中文摘要，并把主/次失败原因写入 trace / request log。

### 4.4 Failover boundary

跨 family 可切换失败包括：

- 无候选 / family 不存在
- 请求模型不在聚合目录
- `401 / 403 / 404 / 408 / 409 / 429 / 5xx`
- timeout / connect error / stream interrupted

不跨 family 切换的失败包括：

- 请求体无效
- 本地协议适配失败
- 其他明确属于客户端输入错误的 `400 / 422`

### 4.5 Model catalog union

当全局优先级开启时：

- 账号 family 的本地模型缓存
- 聚合 family 的可见模型目录

会在 [local_models.rs](D:/code/Codex-Manager/crates/service/src/gateway/request/local_models.rs) 中按 slug 去重合并后返回。

### 4.6 Chinese-first client errors

调整 [mod.rs](D:/code/Codex-Manager/crates/service/src/gateway/mod.rs) 与 [error_response.rs](D:/code/Codex-Manager/crates/service/src/gateway/error_response.rs)：

- Codex / 带内部特征头的客户端：继续优先 raw error
- 普通外部客户端：优先返回中文摘要

对已有 `中文(english_raw)` 结构直接取中文头；对聚合 family 的常见英文失败补业务翻译。

## 5. Data Flow

### 5.1 Request routing

1. local validation 读入请求和 API Key。
2. 如全局优先级关闭：
   - 继续按 `rotationStrategy` 单 family 处理。
3. 如全局优先级开启：
   - 构造账号变体
   - 构造聚合变体
   - 按 `account_first` / `aggregate_first` 执行 family chain
4. 若主 family 失败且命中跨 family failover 条件，则继续执行次 family。

### 5.2 Models listing

1. 读取 API Key 和协议类型。
2. 如全局优先级关闭：
   - 保持现有 aggregate-only / account-only 行为。
3. 如全局优先级开启：
   - 读取账号模型缓存
   - 读取当前协议可用的聚合目录
   - 合并去重后返回

### 5.3 Client error rendering

1. 业务层尽量构造 bilingual 或可识别错误。
2. `error_message_for_client(prefers_raw, message)`：
   - raw client 返回英文原文
   - external client 返回中文优先
3. `terminal_text_response` 不再重复降级消息内容。

## 6. Error Handling

- `aggregate api not found` 对外返回 `未找到可用聚合 API`。
- `aggregate api model not found...` 对外返回 `请求模型不在聚合 API 已选择目录中`。
- timeout 对外返回 `上游请求超时` 或 `聚合 API 请求超时`。
- 双 family 都失败时，错误摘要要体现最终失败，同时日志保留：
  - primary family
  - secondary family
  - primary failure reason
  - secondary failure reason

## 7. Testing Strategy

先写 failing tests，再写实现：

1. family 顺序与跨 family failover 判定。
2. app settings snapshot / patch 能读写全局优先级开关。
3. local models 在全局优先级开启时返回账号 + 聚合并集。
4. `error_message_for_client` 对 external / raw client 行为分流正确。
5. 聚合 runtime 失败类型能触发跨 family 切换。
6. docker 验证 `aggregate_first` 与 `account_first` 的可访问性，至少覆盖“账号不可用时回退到聚合 API 成功”。

## 8. Files Expected To Change

### Backend

- `D:/code/Codex-Manager/crates/service/src/gateway/core/runtime_config.rs`
- `D:/code/Codex-Manager/crates/service/src/gateway/mod.rs`
- `D:/code/Codex-Manager/crates/service/src/gateway/local_validation/mod.rs`
- `D:/code/Codex-Manager/crates/service/src/gateway/local_validation/request.rs`
- `D:/code/Codex-Manager/crates/service/src/gateway/upstream/proxy.rs`
- `D:/code/Codex-Manager/crates/service/src/gateway/upstream/protocol/aggregate_api.rs`
- `D:/code/Codex-Manager/crates/service/src/gateway/request/local_models.rs`
- `D:/code/Codex-Manager/crates/service/src/gateway/tests/error_response_tests.rs`
- `D:/code/Codex-Manager/crates/service/src/app_settings/shared.rs`
- `D:/code/Codex-Manager/crates/service/src/app_settings/gateway.rs`
- `D:/code/Codex-Manager/crates/service/src/app_settings/api/current.rs`
- `D:/code/Codex-Manager/crates/service/src/app_settings/api/patch.rs`
- `D:/code/Codex-Manager/crates/service/src/app_settings/runtime_sync.rs`

### Frontend

- `D:/code/Codex-Manager/apps/src/types/settings.ts`
- `D:/code/Codex-Manager/apps/src/lib/api/normalize.ts`
- `D:/code/Codex-Manager/apps/src/lib/store/useAppStore.ts`
- `D:/code/Codex-Manager/apps/src/app/settings/page.tsx`

### Docs / tests

- `D:/code/Codex-Manager/docs/superpowers/specs/2026-05-01-gateway-global-channel-priority-design.md`
- `D:/code/Codex-Manager/docs/superpowers/plans/2026-05-01-gateway-global-channel-priority.md`
