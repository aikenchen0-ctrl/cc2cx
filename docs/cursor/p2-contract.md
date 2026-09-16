# P2 Cursor 内部契约、输出编码与 Provider 流桥接

状态：P2-0 内部契约、P2-1 输出编码和最小合成 HTTP backend 已实现并通过合成测试；P2-2 的通用 Provider 流桥接、可注入 Provider 边界、OpenAI-compatible Chat SSE source 和严格 Provider factory 已实现并通过合成测试；P2-3a 已增加通过只读 effective current-provider resolver 创建 source 的显式入口，database-backed Harness 已在启动时调用该路径，解析顺序为“有效设备级 settings ID 优先、stale ID 只回退不清理、否则数据库 `is_current`”；P2-3c 已增加共享 HTTP client 上的有界、无请求体 preflight 和失败回滚；模型控制面已补齐 `AvailableModels`、`GetUsableModels`、`GetDefaultModel` 和两个 `GetDefaultModelForCli` 别名，默认模型来自当前 provider 的首个模型 ID。`BidiAppend` 会先验证请求契约，只有没有未实现字段时才写入 `TransportRegistry`；历史、图片、MCP 或 reasoning 字段实际出现时，在创建 transport 和启动 Provider 前返回明确的 `NOT_IMPLEMENTED`。`RunSSE` 可重放合成或显式注入 Provider 的流式帧；`run_provider_stream` 负责把已规范化的 Provider 事件编码到同一 transport，并处理完成、错误、截断、空闲超时和客户端取消；factory 复用 cc2cx 的 ProviderType/ProviderAdapter，只接受显式 `openai_chat`、有效且不带 userinfo 的 URL，以及 Bearer/ClaudeAuth 输入策略（出站统一 Bearer）。尚未接入现有故障转移、真实 Cursor E2E 或动态网络路由。

## 边界

`cursor/protocol` 负责 Connect/BidiAppend 解码；`cursor/adapter.rs` 接收解码后的 `DecodedAppend`，输出不携带 protobuf 类型的内部契约，并只对实际出现的未实现字段生成 `Dropped`/`Unsupported` 记录。`cursor/provider.rs` 定义可取消的 `CursorProvider` 和 OpenAI-compatible Chat SSE source；它只接收规范化 `ProviderInvocation`，负责自身请求构造、认证内存边界和 SSE 转换。`cursor/provider_factory.rs` 负责把一个已读取的 cc2cx `Provider` 显式验证为 OpenAI Chat 形态，并复用现有适配器的 URL/认证解析；其 `from_current` 入口使用只读的 effective current-provider resolver：有效设备级 settings ID 优先，stale ID 只回退数据库 `is_current` 且不修改设备级 settings；它不执行故障转移，但已由 database-backed Harness 调用。`run_provider_stream` 只负责把规范化事件写入 Cursor transport，不负责执行工具或把 Cursor body 交给现有 `proxy/handlers.rs`。

## 独立 backend listener 边界

`CursorProtocolBackend::router()` 仍可作为进程内 Router 用于确定性测试。`CursorProtocolBackend::start(SocketAddr)` 是 loopback-only listener：绑定前拒绝非 loopback 地址，绑定后返回实际监听地址和 URL，`CursorProtocolBackendRuntime::stop()` 负责有界且幂等的 graceful shutdown。standalone 调用者必须显式启动；`AppState` 的 database-backed Harness 会在解析 current provider 后调用它，并把实际地址写入 MITM 配置；不会自动写入 Provider key 或绕过 CA 信任。

当前 listener 验证只证明 loopback 绑定、TCP 可接受、端口释放和非 loopback 拒绝；database-backed Harness 另有启动前无请求体探测、启动/停止/失败回滚测试。Provider 启动前的认证、5xx、超时和传输错误，以及请求后的认证、超时、传输、协议和截断错误均可记录为脱敏健康代码；启动探测失败会在写入 settings/proxy 前阻断，后续请求失败使 `/health` 返回 `503` 并令 Harness 进入 `degraded`。2026-09-09 起，透明入口的合成 HTTP/2 客户端可将白名单路径转到该 backend（不转发 Authorization），未匹配路径 fail-closed。这仍不是真实 Cursor 窗口验收。真实窗口必须按单一入口模式记录（透明 Hosts **或** 显式 `http.proxy`，不可两套同时开着宣称透明入口已通过）。

## 四个契约

| 契约 | 当前字段/事件 | 规则 |
|---|---|---|
| `CursorRequest` | `request_id`、`append_seqno`、`conversation_id`、`model_id`、当前用户消息及消息 ID、`CursorAction::Run`、`field_dispositions` | 首条 Run 保留请求身份、序号、会话、模型和当前消息；能力缺口以 `Dropped`/`Unsupported` 枚举记录；没有模型不在此层猜测 |
| `CursorAction` | `Run` | P2-0 只接受单轮 Run；Exec、KV、心跳、预热等动作显式返回 `unsupported` |
| `ProviderInvocation` | 请求 ID、会话 ID、模型 ID、当前消息及消息 ID、`field_dispositions` | 模型必须非空；携带字段处置结果但不携带 protobuf、账号、Cookie、工具执行上下文或完整请求体 |
| `ProviderStreamError` / `ProviderStreamOutcome` | 分类错误、`Completed`、`Failed`、`Truncated`、`IdleTimeout`、`Cancelled` | Provider 流错误和生命周期结果在桥接边界显式化；客户端 `CancelAction` 会先发布 Connect `canceled` 终止帧，再通过 cancellation token 停止 Provider |
| `CursorOutputEvent` | 文本/思考增量、工具开始、heartbeat、usage、turn ended、结构化错误、`unsupported` | 每个 P2-0 Provider 事件都有显式映射；工具参数增量返回 `unsupported`，不静默丢失 |
| `CursorOutputEncoder` | Connect data frame、usage 聚合、TurnEnded、正常/错误 END_STREAM | usage 只在回合结束时发出；终止帧必须是最后一帧；编码器不执行工具、不发起网络 |

## 字段保留/丢失清单

| Cursor 输入字段 | P2-0 处置 | 原因/后续边界 |
|---|---|---|
| 模型 (`requested_model` / `model_details`) | 保留，前者优先 | 为后续 Provider 选择保留；当前合成 backend 仅验证非空，两者都空时阻断调用 |
| 当前用户消息和消息 ID | 保留 | 单轮合成流的最小输入 |
| 会话 ID | 保留 | 后续会话 registry 和流粘性使用 |
| 历史消息 | 实际出现时标记 `dropped`，并在 backend 前置拒绝 | P2-3b 需要显式 hydrate/计数，不能从当前文本猜历史或继续静默调用 Provider |
| 图片/blob | 实际出现时标记 `unsupported`，并在 backend 前置拒绝 | 需校验 blob/schema 后才能进入 Provider |
| MCP 工具定义 | 实际出现时标记 `unsupported`，并在 backend 前置拒绝 | 需独立 schema 和工具生命周期；当前不执行 |
| reasoning/effort 参数 | 实际出现时标记 `dropped`，并在 backend 前置拒绝 | 需扩展受控字段映射，不能默认丢弃 |
| 取消信号 | `run_provider_stream` 已监听 transport 取消 token；真实 Provider 是否停止由其流实现保证 | HTTP body 断开会使 transport 进入 `Disconnected`，桥接不再发帧；真实 Provider 取消验收留到 P2-3 |
| token 用量 | `ProviderEvent::Usage` 保留并映射 | 仅保留聚合 token 桶，不记录原始提示词或凭证 |
| 错误 | 保留结构化 `code`/`message` | message 必须是已分类、可展示的错误，不放入敏感请求内容 |
| 工具/计划副作用重试策略 | 未实现，标记 `deferred_to_p2-3` | 在收到可安全重放响应前不得自动重试副作用动作 |

## 输出编码约束

- `TextDelta`、`ThinkingDelta`、`Heartbeat` 和工具开始编码为 flag `0` 的 Connect protobuf data frame。
- `ThinkingDelta` 使用 `ThinkingStyle::Default`；工具开始使用准确字段编号的 MCP 兼容占位，只携带调用 ID、名称和兼容标识，不携带工具参数或执行结果。
- `Usage` 不单独生成 wire frame，而由有状态编码器按字段保留最新的非空聚合值，在 `TurnEnded` 中按 `optional int64` 字段发出；部分 usage 更新不会清空此前已知的可选桶。
- 正常结束先发送 `TurnEnded` data frame，再发送 `[2, 0, 0, 0, 2, '{}']` 空成功 END_STREAM。
- Provider 错误和取消错误只发送结构化 END_STREAM；错误码经过有限分类映射，不把原始请求、凭证或响应内容写入终止消息。
- 编码器收到终止后再次写入会返回协议错误；直接使用 `finish()` 仍表示本地成功收尾，而 `run_provider_stream` 对没有显式 `Done` 的 Provider 流发送结构化 `unavailable` 截断错误，不把不完整响应伪装成成功。

## 合成测试

`src-tauri/tests/cursor_adapter.rs` 覆盖：请求身份和消息保留、非 Run 动作显式拒绝、缺失模型阻断、文本/思考/工具开始、工具参数 `unsupported`、heartbeat、usage、正常结束和结构化错误映射；Provider bridge 还覆盖正常 `Done`、Provider 错误事件/错误项、缺失 `Done` 的截断、空闲超时和客户端断开取消，以及 presence-aware 的历史/图片/MCP/reasoning 处置（源码当前 15 项；2026-09-07 本轮 15/15）。`src-tauri/tests/cursor_protocol.rs` 另覆盖 data frame 解码、MCP 占位、usage 聚合、正常/错误/取消终止和终止后状态（当前源码 18 项；本轮 18/18）。测试只使用 `request-fixture-*`、`model-fixture-*` 等合成值。

`src-tauri/tests/cursor_backend.rs` 覆盖最小 HTTP 闭环：合法 `BidiAppend` 启动合成单轮流，或通过显式注入 Provider 启动单轮流；`RunSSE` 返回可解析的增量帧和终止帧，迟到订阅者可重放完整历史；模型目录和默认模型端点返回当前 provider 的合成模型 ID；非法 protobuf、未知 `RunSSE` request、非首条序号、重复单轮 append、unsupported action、存在历史/图片/MCP/reasoning 字段和缺少模型均返回明确错误，且不会留下半成品或 phantom transport；RunSSE body 未读完即丢弃时会传播 `Disconnected`，正常读到 `END_STREAM` 时保留 terminal replay；显式 current-provider backend 构造会复用只读选择和 factory 校验。backend 在创建 transport/provider 前检查字段处置：请求中只要出现 `History`、`Images`、`McpTools` 或 `ReasoningEffort`，即返回 HTTP `NOT_IMPLEMENTED`；unsupported action 同样显式拒绝，字段不得静默丢弃；Provider 认证错误会被记录为脱敏健康代码并使 `/health` 返回 `503`，MITM→backend 会执行 header allowlist。源码当前 31 项；本轮 `31/31`，路由专项 `3/3`。`src-tauri/tests/cursor_provider.rs` 另覆盖 OpenAI-compatible Chat 请求构造、启动前无请求体 preflight、SSE 文本/usage/done、CRLF/空 error、工具身份和参数跨事件重组、HTTP 错误不回显响应体、EOF 缺失 `[DONE]`、请求前取消、userinfo URL 拒绝、显式格式、URL、认证、不兼容 Provider、只读 current-provider 优先级、stale settings 保留和模型路由；源码当前 31 项，2026-09-07 本轮 31/31。listener 用例另外覆盖非 loopback 拒绝、loopback 实际绑定、TCP 接受、显式停止、重复停止、丢弃 runtime 后端口释放；这些用例仍是本机合成 runtime 验证，不是生产接入验收。

## 当前未完成项

- `OpenAiChatProvider` 及 `CursorProviderFactory::from_current` 已可通过 database-backed Harness 读取 cc2cx 数据库的有效 current Provider；Harness 启动时会执行无请求体 preflight。透明入口已能把合成 HTTP/2 白名单请求转入该 backend，但仍未应用现有故障转移策略，也尚未完成真实 Cursor 窗口的 Agent 路径 + Provider endpoint 证据。
- 模型目录和默认模型控制面已能返回当前 provider 的模型 ID；账户、profile、usage、`/auth/poll` 仍未被本地伪造或接管，不能据此绕过 Cursor 套餐门控。`StreamChat`、`StreamEdit`、`StreamReview` 的旧式 RPC 也尚未接入该 backend。
- transport 取消 token 已接入 bridge；`AgentService/Run` 后续帧目前支持 `ConversationAction::CancelAction`、客户端 heartbeat 和 Connect 请求半关闭，未支持的 Agent/工具/KV/交互消息会返回显式 unsupported 终止；历史 hydrate、图片/MCP、完整工具参数和工具结果留到后续 P2 子阶段；真实 Cursor 取消验收仍留到 P2-3。
- `CursorOutputEncoder` 目前只覆盖最小服务端 protobuf 子集；完整 AgentServerMessage、MCP 参数/结果和其它工具变体仍未接入。
- 当前 backend 仅接受 `append_seqno = 0` 的单模型、单轮、无工具副作用、且未携带历史/图片/MCP/reasoning 能力缺口的请求；第二条 append、已启动或已终止 transport 会被拒绝。
- 当前 backend listener 仍是最小开发/测试能力：standalone 需显式启动，database-backed Harness 会按 current provider 先执行 preflight 后启动；它已有启动前失败阻断、请求后 provider 健康状态和脱敏 `/health=503`，但仍没有故障转移、backend capability token 或全局 registry 取消，不能把它写成真实 Cursor backend。
- transport generation 使用单调计数；达到 `u64` 上限时返回 `GenerationExhausted`，不会回绕复用旧 generation。
- `TransportRegistry` 的 history、未来序号缓存和 channel 当前无界；P2 只用于本地合成验证，生产化前必须增加容量上限、TTL、慢消费者策略和指标。
- standalone `CursorProtocolBackend::new()` 的输出帧仍来自 `SyntheticProvider`；database-backed Harness 使用 current-provider 的 OpenAI Chat source，并在启动前执行有限 preflight。两者都尚未完成历史 hydrate、图片/MCP、工具执行、真实 Cursor 自定义 endpoint/key 链路或网络选路。
