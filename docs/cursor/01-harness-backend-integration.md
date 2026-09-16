# 01 Harness Backend 接入

## 目标

将 `CursorProtocolBackend` 接入 database-backed `CursorHarness`，完成启动、实际端口注入、失败回滚和停止释放；standalone/test Harness 仍保留显式 opt-in 行为。

## 前置

- P2-3b presence-aware 拒绝与 P2-3c 启动前 Provider 探测已通过。
- 无数据库的纯测试 Harness 保持默认 `CursorProxyConfig.backend = None`；这是 standalone/test 默认值，不代表 AppState 路径。`AppState` 的 database-backed Harness 在有有效 current provider 时派生 loopback backend。

## 实施

1. 增加 backend 配置来源和状态字段。
2. backend 先于 MITM 启动，使用实际绑定地址。
3. settings 写入失败时停止 backend 与 MITM。
4. stop、崩溃恢复和重复启动保持幂等。

## 退出门

- 显式 backend 生命周期测试通过。
- standalone Harness 行为无回归；database-backed Harness 能从 current provider 启动 backend。
- backend 非 loopback 绑定被拒绝。

## 验证

`cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_commands --test cursor_backend`
