# 04 取消、异常与恢复

## 目标

验证客户端取消、超时、断线、重复请求和 backend 重启时的资源释放与 settings 恢复。

## 前置

- `03-real-cursor-basic-stream.md` 已有可重复真实流。

## 实施

1. 建立取消到 Provider stream 的传播检查。
2. 为超时、缺失终止帧、断流、重复 request_id 建立回归。
3. 验证异常退出后的 stale settings 自动恢复。
4. 禁止对有副作用请求盲目重试。

## 退出门

- 取消耗时有界。
- 无 phantom transport、端口泄漏或坏 settings。

## 验证

`cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_protocol --test cursor_adapter --test cursor_commands`
