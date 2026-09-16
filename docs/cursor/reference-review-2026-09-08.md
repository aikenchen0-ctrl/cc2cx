# Cursor 参考复盘与迁移路线

日期：2026-09-08<br>
范围：`cursor-byok`、`go-mitmproxy`、`cursor-fake`、AICodings Pilot 静态/行为资料<br>
目标：缩短真实 Cursor 自定义 endpoint/key 链路的完成路径；当前验收以无需官方账号为前提，其他客户端集成方式按阶段单独验证。

## 结论先行

当前 `cc2cx` 已经具备本机 CA、显式 MITM、Connect 编解码、最小 `Run`、Provider bridge 和 database-backed Harness。下一步最有价值的工作不是继续扩展零散 RPC，而是把 `cursor-byok` 中已经验证过的五个运行时机制按最小范围移植：

1. request-id 级 transport registry。
2. 有序 append inbox 和 generation 生命周期。
3. 会话 actor，将网络帧与 Provider 运行解耦。
4. `ConversationAction` 的取消、注入上下文和后续用户消息分发。
5. 完整 Provider 事件到 Cursor server message 的可恢复映射。

这些机制直接决定普通对话后的取消、连续消息、断线恢复和工具交互是否可靠；它们比增加更多 unary 控制面响应更接近产品目标。

## Evidence → Finding → Path

### E1：`cursor-byok` 的成熟点是运行时状态机

**Evidence**

- `server/src/cursor/transport/registry.rs` 按 `request_id` 保存 local/upstream route、generation、通知和清理任务。
- `server/src/cursor/transport/handle.rs` 保存会话 ID、父级关系、取消令牌、输出订阅和 admission 生命周期。
- `server/src/cursor/conversation/runtime.rs` 通过 actor 接收有序 append，区分 Run、Exec、KV、InteractionResponse、ConversationAction 和 Disconnect。
- `server/src/cursor/conversation/command.rs` 把 `Append`、`RunFinished`、`Disconnect` 作为明确运行时命令。

**Finding**

当前 `cc2cx` 的 `TransportRegistry` 已能缓存帧和传播断开，`/Run` 后续帧已进入有界读取路径并支持 `CancelAction`；但它还没有完整的会话 actor，因此当前实现可以证明“首轮普通流和取消能返回”，不能证明“同一会话可续写、注入上下文或处理工具结果”。

**Path**

先在 `cc2cx/src-tauri/src/cursor/transport.rs` 增加最小 generation/admission/command 语义，再把 `protocol_backend.rs` 的 `/Run` body reader 接到 actor；不迁移 checkpoint、prompt compiler、MCP 和工具执行代码。

### E2：取消不是 HTTP 连接关闭的同义词

**Evidence**

- `cursor-byok/server/src/cursor/conversation/runtime.rs` 的 `CancelAction` 会取消 active `Run`，并清理运行中的工具。
- `cursor-byok/server/src/cursor/transport/handle.rs` 的 disconnect token 会向下游传播。
- `cc2cx` 当前已有 `TransportHandle::disconnect()` 和 Provider cancellation token，但只覆盖客户端断开/响应流关闭。

**Finding**

此前实现会排空 `ConversationAction::CancelAction`；本轮已补上 proto 解码、取消 token 传播和 canceled 终止帧。

**Path**

`CancelAction` 已作为第一个后续交互能力落地：收到后通过 `cancel_with_frame()` 发出明确的 Connect canceled terminal frame，并验证 Provider cancellation token 被置为 cancelled；客户端 heartbeat 也已保活，其他后续消息会显式返回 unsupported。下一步再处理连续消息和工具副作用边界。

### E3：事件映射已经有可直接复用的最小形状

**Evidence**

- `cursor-byok/server/src/cursor/protocol/events.rs` 已定义 TextDelta、ThinkingDelta、ToolCallStart、Heartbeat、ThinkingCompleted、TurnEnded 和 token usage 的 wire 结构。
- `cc2cx/src-tauri/src/cursor/adapter.rs` 已有等价的 `CursorOutputEvent` 和 `CursorOutputEncoder`，并已覆盖文本、思考、工具开始、usage、错误、终止帧。

**Finding**

事件编码器不是主要缺口；真正缺口是运行时没有持续消费后续消息，也没有把工具参数增量、tool completed 和 shell output 形成可取消的会话状态。

**Path**

保留 `cc2cx` 自己的事件契约，不复制 `cursor-byok` 的 provider 类型；优先补 `ToolCallArgumentsDelta` 的显式 unsupported/可选映射，再实现 tool completed 的生命周期测试。

### E4：Pilot 的高价值部分是传输诊断

**Evidence**

- AICodings Pilot 资料显示 `application/connect+proto`、flags/长度帧、可选解压、半帧处理和双向流是关键行为。
- Pilot 的 `Network Diag` 文案明确区分 bidi 不支持、响应被缓冲、证书不受信和 30 秒总体超时。

**Finding**

帧级解压、低缓冲 flush、诊断分类可以转化为公开、可审计能力；其余行为需要单独完成安全、兼容性和授权评估后再决定是否进入产品。

**Path**

在真实 Cursor 基础流通过后，新增只读、30 秒内、有结构化错误码的诊断模块；诊断只记录阶段耗时、HTTP 版本、ALPN、状态码和错误类别，不记录正文、token、Cookie 或私钥。

### E5：`go-mitmproxy` 与 `cursor-fake` 不能替代 Agent runtime

**Evidence**

- `go-mitmproxy` 提供 CONNECT、动态证书、H1/H2、SSE/WebSocket 转发和 flush/backpressure 参考，但还包含 `SslInsecure`、通用 addon 和 key-log 能力。
- `cursor-fake` 只提供 fake-IP/DNS/TCP 透明转发，不解密 TLS、不解析 protobuf、不理解 request_id。

**Finding**

两者只能约束连接生命周期和旁路实验，不能解决 Cursor 对话状态、取消或工具协议。

**Path**

继续使用 Rust `hudsucker + rcgen` 作为主 MITM；`cursor-fake` 保持 P4 独立实验，默认关闭；不得把 Go runtime 引入产品主进程。

## 推荐迁移顺序

| 顺序 | 工作项 | 直接收益 | 暂不做 |
|---|---|---|---|
| 1 | `/Run` 后续帧分类与有序 inbox | 防止排空导致交互丢失；取消已完成首片 | 工具执行 |
| 2 | `CancelAction` 到 Provider cancellation | **已完成并有回归测试** | 账号控制面 |
| 3 | generation/admission/断线清理 | 防止旧流覆盖新流、重复终止和资源泄漏 | checkpoint |
| 4 | ToolCall 参数/完成事件契约 | 为 Cursor Agent 兼容建立可测边界 | shell 执行 |
| 5 | `AvailableModels` / `GetServerConfig` 最小 fixture | **已完成安全默认和 Provider 模型目录**；仍需真实字段验收 | 其他身份与账户控制面 |
| 6 | 真实 Cursor 隔离 profile E2E | 证明自定义 endpoint/key 真正生效 | 默认 profile |
| 7 | 诊断、选路、H1/H2 A/B | 才能讨论更快更稳定 | CDN 宣称 |

## 明确不迁移

- `cursor-byok` 的 checkpoint、Prompt compiler、MCP、账号服务和完整工具执行树：先作为协议行为参考，不整仓搬运。
- `go-mitmproxy` 的 `SslInsecure`、默认 key log 和任意 addon。
- `cursor-fake` 的系统 DNS/hosts 默认改写。

## 当前阶段判定

- **已证实**：`cc2cx` 的最小 `Run`、gzip、跨 body chunk、`IsConnected`、Provider bridge 和隔离 profile 测试通过。
- **行为证据**：Cursor 3.19.13 使用 HTTP/2、gzip 和 `AgentService/Run` 双向流；Pilot 证明帧级解压、缓冲诊断和连接分类的重要性。
- **待验证**：真实 Cursor 是否在无官方账号时直接进入 `/Run`；`AvailableModels/GetServerConfig` 的实际字段要求；真实取消帧和响应压缩组合。

本文件只作为迁移决策输入，不代表真实 Cursor 已支持。
