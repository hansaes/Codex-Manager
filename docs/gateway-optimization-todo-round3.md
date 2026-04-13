# Gateway 优化 TODO Round 3

更新时间：2026-04-13

本轮目标：统一 gateway 错误文案到 failover / cooldown / unavailable 的 follow-up 分析，减少 `candidate_executor` 与 `response_finalize` 的重复判定。

- [x] 在 `account_status` 抽取共享 gateway error follow-up 分析结果
- [x] 让 `candidate_executor` / `response_finalize` 复用共享分析结果并保持现有行为
- [x] 运行关键测试并记录结果

已执行验证：

- `cargo test -p codexmanager-service classify_account_availability_signal -- --nocapture`
- `cargo test -p codexmanager-service outcome -- --nocapture`
