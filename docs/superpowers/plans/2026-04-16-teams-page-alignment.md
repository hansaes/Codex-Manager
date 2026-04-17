# Teams Page Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 Teams 页面改成账号管理页风格的“搜索在上、展示在下”的管理布局。

**Architecture:** 抽一个无依赖的 Teams 页面过滤 helper 承担搜索与状态筛选逻辑，用 node:test 做回归；页面组件只负责状态、布局和交互。

**Tech Stack:** TypeScript, React, Next.js App Router, Tailwind CSS, shadcn/ui, node:test.

---

### Task 1: 为 Teams 过滤逻辑写失败测试

**Files:**
- Create: `D:\code\Codex-Manager\apps\tests\teams-page-filters.test.mjs`
- Create: `D:\code\Codex-Manager\apps\src\app\teams\teams-page-helpers.ts`

- [ ] **Step 1: 写测试覆盖搜索字段和状态筛选**
- [ ] **Step 2: 运行 `node --test tests/teams-page-filters.test.mjs` 验证失败**
- [ ] **Step 3: 写最小 helper 让测试通过**
- [ ] **Step 4: 再跑一次测试确认通过**

### Task 2: 重构 Teams 页面布局

**Files:**
- Modify: `D:\code\Codex-Manager\apps\src\app\teams\page.tsx`

- [ ] **Step 1: 接入搜索与状态筛选 state**
- [ ] **Step 2: 将顶部改成轻量标题 + 工具栏**
- [ ] **Step 3: 把统计区下沉到表格上方，保持主信息为表格**
- [ ] **Step 4: 保留现有团队操作与成员弹窗行为**

### Task 3: 验证

**Files:**
- Verify only

- [ ] **Step 1: 运行 `node --test tests/teams-page-filters.test.mjs`**
- [ ] **Step 2: 运行 `pnpm run build:desktop`**
- [ ] **Step 3: 检查 Teams 页面无类型错误且布局符合 A 方案**
