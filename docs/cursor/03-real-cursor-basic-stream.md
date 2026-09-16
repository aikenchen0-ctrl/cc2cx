# 03 真实 Cursor 基础流

## 目标

在真实 Cursor 上验收通过 cc2cx 自定义 API endpoint 和 key 的单模型普通对话和增量流，不要求 Cursor 官方账号登录，不扩展工具或历史能力。

## 前置

- database-backed backend 接入、Provider factory 校验、启动前无请求体探测以及请求后的健康/认证错误分类通过。
- 只读确认 Cursor 可执行文件、版本、settings、单实例和监听端口。
- CA 安装必须是独立、可审计动作。

## 实施

1. 在 cc2cx 中选择或配置自定义 endpoint/key；启动 backend、Harness/MITM。
2. 核对 settings 代理地址与实际监听端口。
3. 启动 Cursor，验证请求经本机代理到达自定义 endpoint，记录首 token、终止帧和错误；不得把 key 写入 Cursor settings 或日志。
4. 任一前置条件失败即记录为阻断，不推断协议结论。

## 退出门

- 至少一次可重复普通对话流成功。
- 流式终止、上游错误和连接断开均有记录。

## 验证

保存脱敏聚合结果和手工验收清单，不保存真实 token、Cookie 或用户提示词。
