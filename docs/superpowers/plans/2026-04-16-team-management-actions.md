# Team Management Actions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复团队邀请重复发送误判，并补齐移出成员、撤回邀请和 Teams 页轻量 UI 对齐。

**Architecture:** 后端先把 Team 管理动作拆成可测试的纯辅助函数和明确的 RPC 动作，再由前端扩展 client / hook / modal 进行行级操作。UI 仅对齐现有 glass 风格，不重做页面信息架构。

**Tech Stack:** Rust service RPC, Tauri commands, TypeScript, React, Next.js App Router, TanStack Query, Tailwind CSS, shadcn/ui.

---

### Task 1: 写入团队服务端回归测试

**Files:**
- Modify: `D:\code\Codex-Manager\crates\service\src\team_management.rs`

- [ ] **Step 1: 为邀请归一化与跳过逻辑写 failing tests**

覆盖：
- 输入邮箱去重
- 已加入邮箱跳过
- 已邀请邮箱跳过
- roster 延迟同步时不应 hard fail

- [ ] **Step 2: 运行 lib test 验证失败**

Run: `cargo test -p codexmanager-service team_management --lib`

Expected: 新测试 FAIL，暴露当前邀请逻辑不满足预期。

- [ ] **Step 3: 写最小实现让测试通过**

提取纯辅助函数，减少对真实网络调用的耦合。

- [ ] **Step 4: 再次运行 lib test**

Run: `cargo test -p codexmanager-service team_management --lib`

Expected: PASS

### Task 2: 新增 remove member / revoke invite 服务端动作

**Files:**
- Modify: `D:\code\Codex-Manager\crates\service\src\team_management.rs`
- Modify: `D:\code\Codex-Manager\crates\service\src\rpc_dispatch\team.rs`
- Modify: `D:\code\Codex-Manager\crates\core\src\rpc\types.rs`
- Modify: `D:\code\Codex-Manager\apps\src-tauri\src\commands\team.rs`
- Modify: `D:\code\Codex-Manager\apps\src-tauri\src\commands\registry.rs`

- [ ] **Step 1: 为参数校验和返回结构写 failing tests**

至少覆盖：
- `teamId` 为空时报错
- `userId` 为空时报错
- `email` 为空时报错

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p codexmanager-service team_management --lib`

- [ ] **Step 3: 实现最小后端动作**

新增：
- 删除成员 HTTP DELETE
- 撤回邀请 HTTP DELETE
- RPC dispatch 与 desktop command 暴露

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p codexmanager-service team_management --lib`

### Task 3: 扩展前端 Team API 类型与 client

**Files:**
- Modify: `D:\code\Codex-Manager\apps\src\types\team.ts`
- Modify: `D:\code\Codex-Manager\apps\src\lib\api\normalize.ts`
- Modify: `D:\code\Codex-Manager\apps\src\lib\api\team-client.ts`
- Modify: `D:\code\Codex-Manager\apps\src\hooks\useTeams.ts`

- [ ] **Step 1: 为邀请结果与新动作定义类型**

加入更丰富的 invite result，以及 remove/revoke client。

- [ ] **Step 2: 对应更新 normalize 与 hook**

hook 要能触发：
- invite
- remove member
- revoke invite

- [ ] **Step 3: 运行 TypeScript / build 验证无类型错误**

Run: `pnpm run build:desktop`

Expected: 至少类型层面不再报前端错误。

### Task 4: 重构团队成员弹窗与行级操作

**Files:**
- Modify: `D:\code\Codex-Manager\apps\src\components\modals\team-members-modal.tsx`

- [ ] **Step 1: 先让弹窗派生 joined / invited / duplicate / already-exists 数据**

- [ ] **Step 2: 增加行级按钮**

已加入：
- 移出

待邀请：
- 撤回

- [ ] **Step 3: 增加危险操作确认**

- [ ] **Step 4: 邀请区做轻量 UI 对齐**

### Task 5: 调整 Teams 页与侧边栏图标

**Files:**
- Modify: `D:\code\Codex-Manager\apps\src\app\teams\page.tsx`
- Modify: `D:\code\Codex-Manager\apps\src\components\layout\sidebar.tsx`

- [ ] **Step 1: 替换团队管理导航图标**

- [ ] **Step 2: 调整 Teams 页头部与卡片层次**

参考：
- `D:\code\Codex-Manager\apps\src\app\models\page.tsx`
- `D:\code\Codex-Manager\apps\src\app\accounts\page.tsx`

- [ ] **Step 3: 保持现有表格结构不被大幅打散**

### Task 6: 验证与收尾

**Files:**
- Verify only

- [ ] **Step 1: 运行 service 相关测试**

Run: `cargo test -p codexmanager-service team_management --lib`

- [ ] **Step 2: 运行完整前端构建**

Run: `pnpm run build:desktop`

- [ ] **Step 3: 手动检查 UI 关键路径**

检查：
- 团队页能打开
- 成员弹窗能打开
- 邀请按钮状态正确
- 移出 / 撤回按钮状态正确
- toast 文案合理

- [ ] **Step 4: 确认没有覆盖现有未提交变更**

Run: `git status --short`
