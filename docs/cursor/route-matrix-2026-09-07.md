# Cursor 3.19.13 路由与协议矩阵

生成日期：2026-09-07<br>
审计对象：Windows Cursor `3.19.13`（commit `dd066f332fcea7382764400fde902f61920648d0`）与 cc2cx 当前 Cursor 接入边界。

## 1. 结论

真实 Cursor 日志证明：

- 客户端为 `agent.v1.AgentService/Run` 建立双向流；该方法不是 `RunSSE`。
- 客户端还会调用 `aiserver.v1.AiService/AvailableModels`、`aiserver.v1.ServerConfigService/GetServerConfig`，并频繁调用 `aiserver.v1.BackgroundComposerService/ListAgentStoreDirectory` 等控制面方法。
- Cursor 的 transport factory 明确创建 HTTP/2 transport，ALPN 为 `h2`，发送 gzip、接收 gzip，并启用 10 秒 ping/20 秒 ping timeout/300 秒 idle timeout。
- 日志只证明服务名、方法名、transport 参数和错误分类；没有记录完整 URL、请求头或正文。因此不能从这些日志单独推导每个 RPC 的实际 `Content-Type`，也不能把 `hasToken=true` 当作 token 值或自定义 key 已生效的证据。

cc2cx 当前本地化闭环对白名单中的 `BidiAppend`、`RunSSE` 和最小 `Run` 解密后转发到本地 backend；`Run` 支持首个 Connect 帧跨 HTTP body chunk，以及请求侧 `Content-Encoding: gzip`（有界解压）。其它 Cursor 路径按 fallback 透传或拒绝。生成了 proto 不等于已经提供了对应 HTTP handler。

## 2. 版本与安装资源证据

| 项目 | 证据 |
|---|---|
| Cursor 版本 | `C:\Program Files\cursor\resources\app\product.json:152` 的 `version=3.19.13` |
| Cursor commit | `C:\Program Files\cursor\resources\app\product.json:153` 的 `commit=dd066f332fcea7382764400fde902f61920648d0` |
| VS Code 基线 | `C:\Program Files\cursor\resources\app\product.json:149` 的 `vscodeVersion=1.128.0` |
| 控制面更新地址 | `C:\Program Files\cursor\resources\app\product.json:24` 的 `updateUrl=https://api2.cursor.sh/updates` |
| 统计代理地址 | `C:\Program Files\cursor\resources\app\product.json:20` 的 `statsigLogEventProxyUrl=https://api3.cursor.sh/tev1/v1` |
| 默认用户设置 | `C:\Users\血饮\AppData\Roaming\Cursor\User\settings.json`（本次只读检查未发现 `http.proxy`） |

`product.json` 只描述应用元数据和部分控制面 URL，未提供 BYOK endpoint 配置，也不能替代运行时请求证据。

## 3. 真实 Cursor 行为矩阵

下表的“日志证据”来自 Cursor Structured Logs；请求正文、凭据和用户内容未写入本文。

| 逻辑 RPC | HTTP path（由 proto 规范映射） | 方法形态 | Content-Type | HTTP/版本证据 | 日志/源码证据 | 当前 cc2cx 本地状态 |
|---|---|---|---|---|---|---|
| `AgentService.Run` | `/agent.v1.AgentService/Run` | 双向流 | 真实日志未记录媒体类型；cc2cx 当前按 Connect framed protobuf 接收/返回，支持请求 gzip 解压、跨 body chunk 首帧和 `ConversationAction::CancelAction` | `HTTP/2` transport；ALPN `h2` | Cursor 日志 `C:\Users\血饮\AppData\Roaming\Cursor\logs\20260906T155957\window1_wb0\exthost\anysphere.cursor-always-local\Cursor Structured Logs.20260907T160004_3003aec9.1.log:524-530`；参考 proto `C:\codeDev\cyberWork\aiapiexe\cursor-byok\protocols\cursor\agent_v1.proto:8732-8736` | **已覆盖最小普通流和取消首片**：首个 `AgentClientMessage::RunRequest` 驱动现有 provider adapter；后续用户消息、工具和完整 Agent runtime 仍未实现 |
| `AgentService.RunSSE` | `/agent.v1.AgentService/RunSSE` | 一元请求、服务端流 | 参考实现响应为 `text/event-stream`，并带 `connect-protocol-version: 1` | 当前 cc2cx backend 使用本机 Hyper listener；真实 Cursor transport 仍需确认是否选择该方法 | cc2cx `C:\codeDev\cyberWork\aiapiexe\cc2cx\src-tauri\src\cursor\protocol_backend.rs:502-547`；参考实现 `C:\codeDev\cyberWork\aiapiexe\cursor-byok\server\src\api\cursor\run_sse.rs:20-40` | **已覆盖合成 backend**；真实 Cursor 是否调用仍待验收 |
| `BidiService.BidiAppend` | `/aiserver.v1.BidiService/BidiAppend` | 一元请求/响应；请求内携带 Agent client message | 参考/测试使用 Connect protobuf（常见媒体类型需由真实抓包确认） | 上下文日志显示 Cursor 使用 HTTP/2 transport，但未将该 RPC 的单独版本写入日志 | cc2cx `C:\codeDev\cyberWork\aiapiexe\cc2cx\src-tauri\src\cursor\protocol_backend.rs:374-500`；路由 `C:\codeDev\cyberWork\aiapiexe\cc2cx\src-tauri\src\cursor\routes.rs:19-22`；参考解码 `C:\codeDev\cyberWork\aiapiexe\cursor-byok\support\cursor-capture\decode.go:22-35,164-188` | **已覆盖合成 backend**；真实 Cursor 发送形态、压缩和头部仍待验收 |
| `AiService.AvailableModels` | `/aiserver.v1.AiService/AvailableModels` | 一元请求/响应 protobuf | 未由客户端日志记录；cc2cx backend 返回本地 Provider 模型元数据 | HTTP/2 transport 是全局 transport-factory 证据，不能证明此 RPC 单独连接复用细节 | Cursor 日志 `C:\Users\血饮\AppData\Roaming\Cursor\logs\20260906T155957\window1_wb0\exthost\anysphere.cursor-always-local\Cursor Structured Logs.20260907T160004_3003aec9.log:46`；proto `C:\codeDev\cyberWork\aiapiexe\cursor-byok\protocols\cursor\aiserver_v1.proto:2454-2470,43411-43419`；cc2cx `src-tauri/src/cursor/protocol_backend.rs`、`tests/cursor_backend.rs` | **已覆盖最小本地响应**；只含模型名/agent 能力，真实 Cursor 字段需求仍待验收 |
| `AgentService.GetUsableModels` | `/agent.v1.AgentService/GetUsableModels` | 一元请求/响应 protobuf | 未由客户端日志记录；cc2cx backend 返回本地 Provider 模型元数据 | 同上，仅能继承 transport-factory 的 HTTP/2 证据 | proto `C:\codeDev\cyberWork\aiapiexe\cursor-byok\protocols\cursor\agent_v1.proto:3061-3069,8733-8741`；cc2cx `src-tauri/src/cursor/protocol_backend.rs`、`tests/cursor_backend.rs` | **已覆盖最小本地响应**；真实客户端字段需求仍待验收 |
| `AiService.GetUsableModels`（兼容别名） | `/aiserver.v1.AiService/GetUsableModels` | 一元请求/响应 protobuf | 未由客户端日志记录；cc2cx backend 返回同一模型目录 | 同上 | cc2cx `src-tauri/src/cursor/protocol_backend.rs`、`tests/cursor_backend.rs` | **已覆盖最小本地别名**；参考 proto 中主声明位于 AgentService，别名属于兼容层 |
| `ServerConfigService.GetServerConfig` | `/aiserver.v1.ServerConfigService/GetServerConfig` | 一元请求/响应 protobuf | 未由客户端日志记录；cc2cx backend 返回本地安全默认 | HTTP/2 transport；真实失败日志记录该 service/method | Cursor 日志 `C:\Users\血饮\AppData\Roaming\Cursor\logs\20260907T213619\window3\exthost\anysphere.cursor-always-local\Cursor Structured Logs.log:189`；proto `C:\codeDev\cyberWork\aiapiexe\cursor-byok\protocols\cursor\aiserver_v1.proto:17988-18018,44930-44933`；cc2cx `src-tauri/src/cursor/protocol_backend.rs`、`tests/cursor_backend.rs` | **已覆盖最小本地响应**；真实 Cursor 字段需求仍待验收 |
| `AiService.GetServerConfig`（兼容别名） | `/aiserver.v1.AiService/GetServerConfig` | 一元请求/响应 protobuf | 未由客户端日志记录；cc2cx backend 返回同一安全默认 | 同上 | proto `C:\codeDev\cyberWork\aiapiexe\cursor-byok\protocols\cursor\aiserver_v1.proto:43411-43419`；cc2cx `src-tauri/src/cursor/protocol_backend.rs`、`tests/cursor_backend.rs` | **已覆盖最小本地别名**；需确认是否仍被新客户端使用 |
| `NetworkService.IsConnected` | `/aiserver.v1.NetworkService/IsConnected` | 空请求/空 protobuf 响应 | cc2cx backend 返回 `application/proto`，Connect 单帧空 `IsConnectedResponse` | HTTP/2 transport 的全局证据；cc2cx 通过本地 HTTP handler 响应 | proto `C:\codeDev\cyberWork\aiapiexe\cursor-byok\protocols\cursor\aiserver_v1.proto:21418-21424,44758-44762`；参考 handler `C:\codeDev\cyberWork\aiapiexe\cursor-byok\server\src\api\cursor\handlers.rs:174-188`；cc2cx `src-tauri/src/cursor/protocol_backend.rs`、`tests/cursor_backend.rs` | **已覆盖本地兼容探针**；不读取认证；真实 Cursor 对状态码和 Connect envelope 的容忍度仍待验收 |
| `BackgroundComposerService.ListAgentStoreDirectory` | `/aiserver.v1.BackgroundComposerService/ListAgentStoreDirectory` | 一元请求/响应 protobuf | 未由客户端日志记录 | HTTP/2 transport；日志显示大量重复调用以及 60s deadline/504/timeout 失败 | Cursor 日志 `C:\Users\血饮\AppData\Roaming\Cursor\logs\20260906T155957\window1_wb0\exthost\anysphere.cursor-always-local\Cursor Structured Logs.20260907T160004_3003aec9.log:2-3,1186-1189`；proto service `C:\codeDev\cyberWork\aiapiexe\cursor-byok\protocols\cursor\aiserver_v1.proto:43878-43881`（完整方法定义需从同一 proto 的 BackgroundComposerService 段确认） | **未作为本地兼容 handler**；会 fallback 到官方上游或按策略拒绝 |
| `DashboardService.GetMe` | `/aiserver.v1.DashboardService/GetMe` | 一元请求/响应 protobuf | 未由客户端日志记录 | HTTP/2 transport；日志记录过 TLS 建连失败 | Cursor 日志 `C:\Users\血饮\AppData\Roaming\Cursor\logs\20260907T213619\window3\exthost\anysphere.cursor-always-local\Cursor Structured Logs.log:189` | **未作为本地兼容 handler**；属于官方控制面，不是 BYOK 主路径 |

### 3.1 重要方法名差异

Cursor Structured Logs 中真实 Agent 流使用小写/大写混合的客户端日志标签 `method: "Run"`/`method: "run"`，但 proto 的规范方法名是 `Run`，HTTP path 应保持大写 `Run`。这不是两个 RPC。cc2cx 不能通过把 `/Run` 简单改名为 `/RunSSE` 来兼容：`Run` 是客户端和服务端双向流，`RunSSE` 是一元 request-id 加服务端流，两者消息方向和 framing 不同。

## 4. HTTP、Connect framing 与 Content-Type 证据边界

### 已证实

1. Cursor 3.19.13 transport factory 使用 HTTP/2、ALPN `h2`、TLS 最低版本 `TLSv1.2`、gzip 压缩和连接 ping。证据：`C:\Users\血饮\AppData\Roaming\Cursor\logs\20260906T155957\window1_wb0\exthost\anysphere.cursor-always-local\Cursor Structured Logs.20260907T160004_3003aec9.1.log:72-77`，以及新一轮日志 `C:\Users\血饮\AppData\Roaming\Cursor\logs\20260907T213619\window3\exthost\anysphere.cursor-always-local\Cursor Structured Logs.log:35-42`。
2. `RunSSE` 在参考服务端以 `text/event-stream` 返回 Connect-framed bytes，并带 `connect-protocol-version: 1`：`C:\codeDev\cyberWork\aiapiexe\cursor-byok\server\src\api\cursor\run_sse.rs:20-40`。
3. `BidiAppend` 请求是 Connect envelope 内的 protobuf；参考捕获工具按 path 解码 `BidiAppendRequest` 并进一步解码 `AgentClientMessage`：`C:\codeDev\cyberWork\aiapiexe\cursor-byok\support\cursor-capture\decode.go:164-188,200-223`。
4. cc2cx 本地 backend 的 `Run` 输出是 `application/connect+proto`，`RunSSE` 输出是 `text/event-stream`，`BidiAppend` 返回 Connect response：`C:\codeDev\cyberWork\aiapiexe\cc2cx\src-tauri\src\cursor\protocol_backend.rs`。

### 尚未证实

- 真实 Cursor 3.19.13 对每个 unary/stream RPC 发出的确切 `Content-Type`（例如 `application/connect+proto`、`application/proto` 或其它 Connect 变体）。
- `Run` 双向流在真实客户端上的 path、请求/响应帧序列和是否经过 `BidiAppend` + `RunSSE` 组合。
- 本地 MITM/hudsucker 是否在客户端 HTTP/2 到 backend 之间保持 HTTP/2；当前 cc2cx backend listener/Hyper handler 的生产协商不能由合成测试推断。
- gzip 是 HTTP content encoding 还是 Connect codec header 在每个 RPC 上的实际组合。
- cc2cx 当前仅对 `Run` 请求侧 `Content-Encoding: gzip` 做有界解压；真实客户端的其它 RPC 压缩组合和响应侧压缩仍待抓包确认。

## 5. cc2cx 覆盖缺口

1. **`/agent.v1.AgentService/Run` 已进入最小覆盖，但仍是首要验收项。** cc2cx 现在能接收首个 `AgentClientMessage::RunRequest`、处理请求半关闭和 `CancelAction` 并桥接 provider 流；后续用户消息、`Exec/Kv/InteractionResponse` 消息仍未实现，不能声称完整 Agent 能力。
2. **模型和 server config handler 可能只是兼容壳。** cc2cx 已注册 `AvailableModels`、`GetUsableModels`、`GetServerConfig` 和 `IsConnected`，但响应字段是否覆盖 Cursor 3.19.13 当前读取的字段尚未由真实请求/响应 fixture 证明。
3. **控制面请求不能全部本地化。** 日志中的 `BackgroundComposerService/ListAgentStoreDirectory`、`DashboardService/GetMe`、认证、统计和 membership 请求仍会走 fallback。应明确区分“BYOK 主链路必要 RPC”和“官方控制面可降级 RPC”，避免为了本地化而伪造账号/会员语义。
4. **HTTP/2 性能结论尚不成立。** Cursor 客户端使用 H2 不等于 MITM 到本地 backend 或 backend 到自定义 provider 也使用 H2；需要分段测量连接协商、TTFB、首 token、流间隔、取消时延和错误率。

## 6. 建议新增测试清单

以下是建议加入的测试项，不包含真实凭据或用户内容。

### P0：协议与路由正确性

- `Run` path 不应被误判为 `RunSSE`；加入 `is_local_path`/分类测试，明确当前决策是 `Passthrough` 或 `Reject`，直到双向流实现完成。
- 为 `/agent.v1.AgentService/Run` 建立协议探针：至少验证双向 stream 的 Connect framing、首条 `RunRequest`、服务端首片、正常结束、错误结束和客户端取消。
- 对 `RunSSE`、`BidiAppend`、`IsConnected` 分别断言 method、path、content-type、`connect-protocol-version` 和 terminal frame；不要用一个通用 JSON handler 覆盖三者。
- 为 `AvailableModels`、`GetUsableModels`、`GetServerConfig` 添加 protobuf response fixture，断言关键字段存在/缺失时的兼容行为和未知字段保留策略。
- 为 `/aiserver.v1.AiService/*` 与 `/agent.v1.AgentService/*` 兼容别名增加一一对应测试，防止只注册其中一个 service name。

### P1：真实客户端行为回放

- 从不含正文/凭据的 Cursor capture 中记录每个 path 的 method、HTTP status、content-type、content-encoding、HTTP version、`connect-protocol-version` 和帧计数。
- 用 Cursor 3.19.13 隔离 profile 启动后，先观察模型刷新和 server config，再发送最小无工具消息；验收必须证明请求实际经过 `Cursor → MITM → local backend → custom provider`。
- 记录 `AgentService/Run` 的首帧、后续帧、终止帧和取消行为；当前只允许把“最小普通流 + CancelAction”作为通过条件，完整 Agent 能力仍需后续交互帧实现。
- 对 `ListAgentStoreDirectory` 等控制面请求验证透传/降级，不把其 `403/504/timeout` 误报为 BYOK provider 故障。

### P2：传输与性能

- 分别测量客户端→MITM、MITM→backend、backend→provider 三段的 HTTP 版本和 ALPN；不能用 Cursor 日志中的 H2 单点证据替代全链路测量。
- H2 与 H1.1 对照：固定 payload 和并发数，记录连接建立、TTFB、首 token、流完成、取消传播和错误率。
- 验证 gzip/identity 两种 codec、半帧、跨 TCP chunk、背压和慢消费者，不允许因缓冲聚合破坏增量输出。
- 复现客户端取消与网络断开，断言 provider cancellation token、transport 状态、RunSSE terminal frame 和 backend listener 资源最终释放。

## 7. 不应作出的结论

- 不能因为 `AvailableModels`/`GetServerConfig` 等 route 已注册，就声称真实 Cursor 已支持自定义 endpoint/key。
- 不能因为日志含 `hasToken=true`，就输出或推断真实 token、Cookie、用户提示词或 key。
- 不能因为客户端 transport 日志为 HTTP/2，就声称 cc2cx 已实现端到端 HTTP/2 或已获得性能提升。
- 不能把 `RunSSE` 的合成测试通过等同于真实 Cursor `Run` 双向流已通过。

## 8. 参考项目行号锚点

| 参考项目 | 行为 | 证据 |
|---|---|---|
| cursor-byok | local proxy 仅按 Cursor host 和精确 path 选择本地化；参考实现公开 `RunSSE`、`BidiAppend` 以及若干兼容控制面路径，完整 `Run` 由其协议 schema/运行时提供 | `C:\codeDev\cyberWork\aiapiexe\cursor-byok\server\src\local_app\proxy.rs:152-191`、`C:\codeDev\cyberWork\aiapiexe\cursor-byok\protocols\cursor\agent_v1.proto:8733-8736` |
| cursor-byok | settings 事务写入代理、系统证书和恢复 | `C:\codeDev\cyberWork\aiapiexe\cursor-byok\server\src\local_app\settings.rs:61-91` |
| cursor-byok | BidiAppend request-id、seqno、hex Agent message 解码 | `C:\codeDev\cyberWork\aiapiexe\cursor-byok\server\src\api\cursor\bidi.rs:156-180` |
| cursor-byok | RunSSE 按 request-id 重放 transport 输出并返回 event-stream | `C:\codeDev\cyberWork\aiapiexe\cursor-byok\server\src\api\cursor\run_sse.rs:20-40,43-65` |
| go-mitmproxy | CONNECT 拦截/直通分支和双向关闭语义 | `C:\codeDev\cyberWork\aiapiexe\go-mitmproxy\proxy\entry.go:220-309`、`proxy\websocket.go:155-228` |
| cursor-fake | fake-IP/DNS/TCP 旁路不解密 TLS、不解析 Cursor protobuf | `C:\codeDev\cyberWork\aiapiexe\cursor-fake\main.go:213-270,399-502` |

本文只记录协议和路由审计结果；真实 Cursor E2E、`/Run` 双向流、端到端 HTTP/2 性能和自定义 endpoint/key 生效仍属于待验证项目。
