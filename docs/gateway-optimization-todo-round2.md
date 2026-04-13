# Gateway 优化 TODO Round 2

更新时间：2026-04-13

本轮目标：先抽取无副作用的 failover helper，统一状态判定入口，不改变现有副作用和回包行为。

- [x] 抽取共享 failover helper，统一 fallback 与非官方上游的状态分类/跟随动作
- [x] 让 `outcome` / `fallback_branch` 接入共享 helper 并保持现有行为
- [x] 运行关键测试并记录结果

已执行验证：

- `cargo test -p codexmanager-service failover_policy -- --nocapture`
- `cargo test -p codexmanager-service outcome -- --nocapture`
- `cargo test -p codexmanager-service fallback_branch -- --nocapture`
