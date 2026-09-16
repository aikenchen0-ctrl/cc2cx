# 02 Backend 健康与认证

## 目标

为 backend 增加可观测健康状态、能力声明和认证边界，避免 MITM 指向已启动但不可用的 backend。

## 前置

- `01-harness-backend-integration.md` 退出门全部通过。

## 实施

1. 定义健康检查和版本/能力响应。
2. backend/provider 认证失败必须在启动或首请求前显式返回。
3. 不在日志中记录 token、Cookie、用户内容或 CA 私钥。
4. 健康失败时回滚 settings 和已启动 runtime。

当前已完成：`GET /health` 返回版本化协议能力，明确标记单轮流式/取消与历史、图片、MCP、reasoning、工具执行能力；同时区分 standalone synthetic backend 与 database-backed/provider backend，但不返回凭据、endpoint 或 provider 详情。Database-backed Harness 启动前对 OpenAI-compatible Provider 发有界、无请求体的 `GET` 探测：任意 HTTP 响应证明网络可达，401/403、5xx、超时和传输错误分别转成脱敏机器可读错误；探测失败发生在 proxy/settings 写入前。Provider-backed 响应还报告最近一次请求的脱敏错误代码；请求后的认证、超时、传输、协议或截断失败会返回 `503`/`degraded`，成功完成会恢复为 `200`/`healthy`。MITM 转发到本地 backend 时仅保留自有协议头 allowlist。

## 退出门

- 配置缺失、factory 拒绝、启动前 Provider 401/403、5xx、超时/传输失败、请求/流超时均有确定错误；启动前探测失败阻断 proxy/settings 写入并释放已启动 runtime；Provider 请求后的认证/超时/传输/协议失败会联动 backend `/health` 和 Harness `degraded`。
- 状态 API 能区分 disabled、starting、running、degraded。
- `/health` 路由与 backend 当前源码共有 17 个测试项，2026-09-07 本轮 `cursor_backend` 通过 17/17；Provider 启动前探测、认证/超时的流内分类、健康联动和 Harness 失败回滚已有专项覆盖。

## 验证

运行 backend、provider、commands 专项测试及 `git diff --check`。
