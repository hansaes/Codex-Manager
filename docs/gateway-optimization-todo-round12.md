# Gateway 优化 TODO Round 12

更新时间：2026-04-13

本轮目标：拆分 `transport.ts` 中的 runtime 探测与缓存加载逻辑，进一步收窄 transport 入口职责。

- [x] 新建 transport runtime 模块
- [x] 让 `transport.ts` 复用并 re-export runtime loader
- [x] 运行关键前端验证并记录结果

本轮验证：

- `pnpm test:runtime`
- `pnpm build:desktop`
