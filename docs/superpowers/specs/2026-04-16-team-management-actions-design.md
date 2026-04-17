# Team Management Actions Design

**Date:** 2026-04-16  
**Scope:** 修复团队邀请的重复发送误判，并补齐团队成员管理动作与 Teams 页风格对齐。

## 1. Goal

为 `Teams` 页面补齐三个能力：

1. 团队邀请不再因为上游列表延迟而误判失败，减少重复发送。
2. 支持对已加入成员执行“移出团队”，对待接受邀请执行“撤回邀请”。
3. 将团队页与成员弹窗的视觉风格对齐到现有的 glassmorphism 数据管理页面。

## 2. Current Problems

### 2.1 邀请成功后被误判失败

当前 `crates/service/src/team_management.rs` 在 `team/invite` 成功后会立刻再次拉取成员/邀请列表，并要求刚邀请的邮箱立刻出现在 roster 中；如果上游存在同步延迟，就会直接报错。用户看到失败后再次点击，容易造成重复发送。

### 2.2 成员管理动作缺失

当前前后端只支持：

- 同步团队
- 查看成员
- 发送邀请
- 移出团队管理

但不支持：

- 将已加入成员移出团队
- 将待接受邀请撤回

### 2.3 团队页面风格弱于其他管理页

`apps/src/app/teams/page.tsx` 与 `apps/src/components/modals/team-members-modal.tsx` 信息层级较薄，和 `Models / Accounts / Settings` 页相比，视觉和交互反馈不够统一。

## 3. Proposed Approach

### Approach A — 最小修复

- 修复邀请误判
- 新增移出成员 / 撤回邀请接口
- 前端仅补按钮

优点：改动最小。  
缺点：UI 风格问题仍明显。

### Approach B — 推荐

- 包含方案 A
- 扩展邀请结果，返回更明确的 invited / skipped / pending-sync 反馈
- 成员弹窗补本地去重、状态提示、危险操作确认
- Teams 页做轻量视觉重构，与现有页面风格对齐

优点：功能和体验一起补齐，范围仍可控。  
缺点：比最小修复需要更多前后端联动修改。

### Approach C — 大幅重构

- 整体重写 Teams 页交互模型
- 加入筛选、批量操作、更多状态面板

优点：扩展性最好。  
缺点：超出当前需求。

**Decision:** 采用 **Approach B**。

## 4. Architecture

### 4.1 Backend service

在 `crates/service/src/team_management.rs` 中：

- 将 Team 上游操作抽成更明确的动作边界：
  - 发送邀请
  - 拉取已加入成员
  - 拉取待接受邀请
  - 删除已加入成员
  - 撤回待接受邀请
- `invite_managed_team_members` 不再把“列表尚未同步可见”视为硬失败。
- 新增：
  - `remove_managed_team_member(team_id, user_id)`
  - `revoke_managed_team_invite(team_id, email)`

### 4.2 RPC / desktop bridge

在 service RPC 与 Tauri commands 中新增：

- `team/removeMember`
- `team/revokeInvite`
- `service_team_remove_member`
- `service_team_revoke_invite`

### 4.3 Frontend data model

扩展团队邀请结果结构，使前端能更清晰地展示：

- 实际提交邀请数量
- 因重复、已加入、已邀请而跳过的邮箱
- 可能仍在同步中的邮箱

成员列表仍沿用 joined / invited 两类，但在 UI 上补足行级动作。

### 4.4 Frontend UI

Teams 页：

- 调整页面头部，加入 badge / title / summary 风格
- 保留现有表格主体，避免过度重构
- 团队导航图标与账号管理图标区分

成员弹窗：

- 邀请区改为更明确的卡片化输入区
- 已加入成员表格：新增“移出”
- 待接受邀请表格：新增“撤回”
- 所有危险操作使用二次确认

## 5. Data Flow

### 5.1 Invite flow

1. 前端先对输入邮箱做 trim + lowercase + 去重。
2. 前端基于当前列表标记已加入 / 已邀请邮箱，避免无意义重复提交。
3. 后端再次做规范化与去重，确保服务端幂等。
4. 后端发送邀请。
5. 后端允许短暂同步延迟：
   - 若接口本身返回成功，则以“已受理”为主
   - 若短时间内仍未在 roster 中看到，只标记为 pending sync，不直接报错
6. 前端显示摘要，并刷新团队与成员列表。

### 5.2 Remove member flow

1. 用户在 joined 行点击“移出”。
2. 前端弹出确认。
3. 前端调用 `removeMember(teamId, userId)`。
4. 后端请求 `/accounts/{team_account_id}/users/{user_id}` 删除成员。
5. 完成后同步团队，并刷新成员列表。

### 5.3 Revoke invite flow

1. 用户在 invited 行点击“撤回”。
2. 前端弹出确认。
3. 前端调用 `revokeInvite(teamId, email)`。
4. 后端请求 `/accounts/{team_account_id}/invites`，以 email 作为删除参数。
5. 完成后同步团队，并刷新成员列表。

## 6. Error Handling

- `teamId` / `userId` / `email` 缺失时立即返回明确错误。
- “邀请已被受理但列表尚未刷新”不再作为 hard error。
- 撤回邀请时，如果目标邀请已不存在，按幂等成功处理更友好。
- 移出成员时，如果用户已不在团队中，应给出清晰错误或幂等成功提示。
- 前端 toast 优先展示业务摘要，不直接暴露生硬的上游校验文案。

## 7. Testing Strategy

先写 failing tests，再改实现：

1. 归一化邀请邮箱时会去重。
2. 已加入 / 已邀请邮箱不会再次发送。
3. 邀请成功但未立即出现在 roster 时，不会直接判定失败。
4. 撤回邀请与移出成员的参数校验与结果归一化正确。
5. 前端成员弹窗本地派生数据（joined / invited / duplicates / existing）正确。

## 8. Files Expected To Change

### Backend

- `D:\code\Codex-Manager\crates\service\src\team_management.rs`
- `D:\code\Codex-Manager\crates\service\src\rpc_dispatch\team.rs`
- `D:\code\Codex-Manager\apps\src-tauri\src\commands\team.rs`
- `D:\code\Codex-Manager\apps\src-tauri\src\commands\registry.rs`
- `D:\code\Codex-Manager\crates\core\src\rpc\types.rs`

### Frontend

- `D:\code\Codex-Manager\apps\src\types\team.ts`
- `D:\code\Codex-Manager\apps\src\lib\api\normalize.ts`
- `D:\code\Codex-Manager\apps\src\lib\api\team-client.ts`
- `D:\code\Codex-Manager\apps\src\hooks\useTeams.ts`
- `D:\code\Codex-Manager\apps\src\components\layout\sidebar.tsx`
- `D:\code\Codex-Manager\apps\src\components\modals\team-members-modal.tsx`
- `D:\code\Codex-Manager\apps\src\app\teams\page.tsx`

### Tests / docs

- `D:\code\Codex-Manager\crates\service\src\team_management.rs`（或其测试模块）
- `D:\code\Codex-Manager\docs\superpowers\plans\2026-04-16-team-management-actions.md`

## 9. Non-Goals

- 本轮不做批量撤回/批量移出。
- 本轮不重构整个 Teams 页面为全新信息架构。
- 本轮不引入新的状态管理库或 UI 框架。
