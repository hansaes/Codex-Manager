# Teams Page Alignment Design

**Date:** 2026-04-16  
**Scope:** 将 Teams 页面信息架构对齐到账号管理页，改成“上方搜索/筛选，下方结果展示”的管理页布局。

## Goal

让团队管理页从偏展示型头图 + 表格，调整为更适合日常运维的管理型页面：

1. 顶部只保留轻量标题与简短说明。
2. 搜索/筛选工具栏上移到页面主入口。
3. 团队信息展示区放到工具栏下方，保持表格为主体。
4. 统计信息弱化成辅助信息，不再抢主视觉。

## Chosen Approach

采用 **Approach A**：

- 弱化现有 hero 区
- 新增账号管理风格的搜索/筛选工具栏 card
- 将团队表格作为主内容区
- 保留现有操作：同步、查看成员、移出

## UI Structure

1. 服务未连接提示卡
2. 轻量页面标题区
3. 搜索/状态筛选/刷新工具栏 card
4. 列表摘要信息区
5. 团队表格 card

## Data & Interaction

- 搜索匹配字段：`teamName`、`sourceAccountLabel`、`sourceAccountId`、`teamAccountId`
- 状态筛选支持：全部 / 可用 / 已满 / 已过期
- 页面保留双击打开成员视图
- 空状态与加载态沿用当前表格风格

## Testing

- 先为页面过滤 helper 写测试，验证搜索与状态筛选行为
- 再落地页面结构改造
- 最后运行前端测试与 `pnpm run build:desktop` 验证
