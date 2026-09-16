# Cursor 改造 P0 源码映射

生成日期：2026-09-04<br>
用途：固定参考版本、源码锚点和迁移边界。本文记录冻结审计结果与阶段状态；已实现的协议/Provider 探针不等于完整 Cursor 功能已经实现。

## 1. 版本冻结

| 项目 | 本地路径 | 分支 | 固定 commit | 工作树 |
|---|---|---|---|---|
| `cc2cx` | `C:\codeDev\cyberWork\aiapiexe\cc2cx` | `main` | `40b3bcd80fd787ba1f2c11197783409a76a1657a` | dirty (Cursor P1 changes) |
| `cursor-byok` | `C:\codeDev\cyberWork\aiapiexe\cursor-byok` | `main` | `2f3bffd15ac2b67b8808c3a2eda6abc5611b1b2a` | clean |
| `go-mitmproxy` | `C:\codeDev\cyberWork\aiapiexe\go-mitmproxy` | `main` | `3f5b4f8ee19d58784c6332ec946f46d3c2b2e73b` | clean |
| `cursor-fake` | `C:\codeDev\cyberWork\aiapiexe\cursor-fake` | `master` | `4d93f83fbd581e4c7632f527eb8b0b9fb4361943` | clean |

许可证以各仓库的 `LICENSE` 为准。当前未将任何参考仓作为 `cc2cx` 的 Git 子模块或运行时依赖。

## 2. 当前主仓结构

| 文件 | 行数 | 当前职责 | P0 结论 |
|---|---:|---|---|
| `src-tauri/src/lib.rs` | 2410 | Tauri 入口、状态注册、命令注册、启动恢复 | 只增加 Cursor 模块注册点 |
| `src-tauri/src/proxy/mod.rs` | 55 | 现有代理模块导出 | 保持 CLI/Provider 网关边界 |
| `src-tauri/src/proxy/server.rs` | 628 | Hyper HTTP/1.1 代理服务器 | 不直接接 Cursor protobuf |
| `src-tauri/src/proxy/handlers.rs` | 3581 | 现有 JSON/Provider handler | 不接收未经适配的 Cursor body |
| `src-tauri/src/services/proxy.rs` | 10003 | 现有代理服务生命周期、接管、故障转移 | Cursor harness 不并入此类的协议逻辑 |
| `src-tauri/src/settings.rs` | 1216 | cc2cx 应用设置 | Cursor 设置另有备份和恢复边界 |
| `src-tauri/src/app_config.rs` | 1250 | 应用类型和配置模型 | 只有需要 UI/配置持久化时才扩展 |
| `src-tauri/Cargo.toml` | 121 | Rust 依赖 | 已有 `tokio`、`hyper`、`reqwest`、`rustls`，P1 已加入 `hudsucker`、`rcgen`、证书解析和校验依赖 |
| `package.json` | 93 | 前端脚本和依赖 | 现有 React/Vite/Vitest 入口可复用 |

当前主仓已具备 `start_proxy_server`、`stop_proxy_server`、`stop_proxy_with_restore` 和 `get_proxy_status` Tauri 命令，以及现有 Provider 级故障转移。它们服务于现有 CLI/JSON 代理，不等于 Cursor IDE 接入。

## 3. 参考项目映射

### 3.1 `cursor-byok`：主要移植参考

| 文件 | 行数 | 读取目的 | 迁移策略 |
|---|---:|---|---|
| `server/src/local_app/mod.rs` | 234 | `CursorHarness` 状态、启动顺序、停止清理 | 只移植生命周期思想和接口形状 |
| `server/src/local_app/proxy.rs` | 222 | `hudsucker`、Cursor host 判定、CONNECT/TLS、path 分流 | 在 `cc2cx/src-tauri/src/cursor/mitm.rs` 和 `routes.rs` 重写 |
| `server/src/local_app/settings.rs` | 109 | 用户级 `settings.json` 读写和代理键清除 | 参考行为，增加原值备份、外部修改检测和原子恢复 |
| `server/src/local_app/ca/mod.rs` | 239 | CA 状态机、生成、检测、安装接口 | 在 `cursor/ca.rs` 重写，不能复制密钥或路径假设 |
| `server/src/local_app/ca/windows.rs` | 75 | Windows 信任库安装/卸载 | 作为 Windows 首个实现；本仓在独立安装动作中管理当前用户 CA，启动不隐式安装 |
| `server/src/api/cursor/bidi.rs` | 211 | `BidiAppend`、`request_id`、顺序和协议错误 | 已移植首片 wire subset，并由 `cursor_protocol`/`cursor_adapter` 专项测试覆盖 |
| `server/src/api/cursor/run_sse.rs` | 232 | `RunSSE`、可重放 transport 输出和生命周期 | 已移植 framing/终止帧、先到时等待和断开保护；完整 Agent transport 生命周期仍待实现 |
| `server/src/api/cursor/proxy.rs` | 227 | Cursor 代理适配和上游边界 | P2 参考，不接入现有 JSON handler |
| `server/src/cursor/**` | 约 18k 行 | Agent、工具、会话、transport、上下文 | P2 单独评估进程内 adapter 或 sidecar |
| `protocols/cursor/**` | 6 文件，约 52979 行 | protobuf 定义 | 只选用实际需要的协议定义，先固定版本 |

关键源码事实：`is_cursor_host` 与 `is_local_path` 是两个独立判断；本地路由不能由“所有 Cursor 请求”概括。`BidiAppend` 是 protobuf，必须保留 `request_id` 和顺序语义。

#### 3.1.1 协议与运行时行号锚点

以下锚点均来自冻结 commit `2f3bffd15ac2b67b8808c3a2eda6abc5611b1b2a`，用于 P2 adapter 的逐项复核：

| 文件与行号 | 已证实行为 | 对 `cc2cx` 的约束 |
|---|---|---|
| `server/src/api/cursor/handlers.rs:49-50` | `RunSSE` 与 `BidiAppend` 是两个独立 endpoint | 不能合并成一个 JSON handler |
| `server/src/api/cursor/handlers.rs:174-188` | `NetworkService/IsConnected` 返回空成功响应，避免慢流被误判断网 | 本地 BYOK 路径必须提供等价探针或明确回退 |
| `server/src/api/cursor/handlers.rs:191-224` | `RunSSE` 先按 `request_id` 等待路由；本地订阅 transport，官方路由透传上游流 | 路由决策先于流订阅，且上游流不能被重新编码成普通 JSON |
| `server/src/api/cursor/handlers.rs:228-323` | `BidiAppend` 先解 Connect/protobuf，再用首条模型选择 Local/Upstream；后续按 `request_id` 绑定 | 首条消息、后续消息和父级请求头必须分开校验 |
| `server/src/api/cursor/handlers.rs:371-392` | 子代理必须同时提供 `x-parent-request-id` 与 `x-parent-agent-tool-call-id` | 缺一即协议错误，不允许静默降级 |
| `server/src/api/cursor/bidi.rs:23-64` | 模型来自 `requested_model` 或 `model_details`；可识别会话 ID、后台任务完成 | adapter 至少保留模型、会话、动作类别 |
| `server/src/api/cursor/bidi.rs:156-179` | `request_id` 必填；当前实现拒绝 `data_binary`，要求十六进制 `data` 解码为 `AgentClientMessage` | fixture 必须覆盖缺 ID、二进制分支、非法 hex 和空消息 |
| `server/src/api/cursor/bidi.rs:182-210` | append 按 `append_seqno` 投递，设置会话/父级关系；心跳产生服务端 heartbeat 事件 | 顺序号和心跳不能在转换层丢失 |
| `server/src/api/cursor/run_sse.rs:20-40` | 本地 `RunSSE` 重放历史帧，并返回 `text/event-stream`、`connect-protocol-version: 1` | 输出层要支持迟到订阅者重放与 Connect 帧 |
| `server/src/api/cursor/run_sse.rs:43-66,127-161` | 以 END_STREAM 标志判断终止；上游按 chunk 转发并按 generation 清理 | 终止帧、断流和旧 generation 覆盖必须有测试 |
| `server/src/cursor/protocol/events.rs:12-49` | Provider `TextDelta`、`ThinkingDelta`、`ToolCallStart` 映射为 `AgentServerMessage`；工具参数增量当前不直接发出 | P2 不能声称完整保留工具参数流 |
| `server/src/cursor/protocol/events.rs:52-79,163-171` | thinking 完成、turn 结束、token 用量和交互包装有明确 wire 结构 | Provider 事件到 Cursor 事件必须有显式契约 |
| `server/src/cursor/conversation/runtime.rs:345-392` | Interaction/KV/ConversationAction 分属不同路径；多个动作仍未实现 | 未捕获动作必须返回显式协议错误或保持上游 |
| `server/src/cursor/conversation/runtime.rs:394-495` | 活跃 Run 可接收用户消息、Cancel、InjectContext、CancelSubagent；取消会终止运行工具 | `cc2cx` 当前已先实现 `CancelAction` 到 Provider cancellation；后续用户消息、InjectContext、CancelSubagent 和工具清理仍需会话 actor |
| `server/src/cursor/compile/run.rs:81-163` | 会话状态先 hydrate；模型配置、动态 MCP 和 subagent 开关随后生效 | adapter 需区分历史、当前用户消息、模型配置和工具定义 |
| `server/src/cursor/compile/run.rs:424-512` | UserMessage、后台任务、ExecutePlan、Summarize 有不同投影；其他动作不会自动获得语义 | 不能把所有 ConversationAction 都当作普通用户文本 |
| `server/src/cursor/compile/run.rs:514-559` | ExecutePlan 从内容/URI 形成批准计划上下文和稳定事件 ID | 计划执行属于有副作用动作，默认不可盲目重试 |
| `server/src/provider/mod.rs:26-34` | Provider 接口是 `stream(invocation, cancellation) -> ProviderStream` | `cc2cx` 已在 `cursor/provider.rs` 提供可取消 bridge 和无请求体 preflight，并由 `settings::get_effective_current_provider_readonly` 为 `cursor/provider_factory.rs::from_current` 解析 current provider；stale 本地 ID 只回退、不清理；database-backed Harness 已调用该 factory；启动前/请求后的健康错误已可联动脱敏状态，故障转移仍待完成 |

### 3.1.3 `cc2cx` P2-0 契约落点

| 文件 | 当前职责 | 边界 |
|---|---|---|
| `src-tauri/src/cursor/adapter.rs` | `CursorRequest`、`CursorAction`、`ProviderInvocation`、`CursorOutputEvent`、`FieldDisposition` 与合成事件映射 | 只接收已解码语义；能力缺口以 `Dropped`/`Unsupported` 记录；不发网络、不选 Provider、不执行工具、不穿透现有 JSON handler |
| `src-tauri/tests/cursor_adapter.rs` | 合成请求、动作拒绝、模型校验和事件映射测试 | 不含真实 token、Cookie、提示词、路径、图片或 CA 私钥 |
| `src-tauri/src/cursor/provider.rs` | `CursorProvider`、可取消 Provider stream 和 OpenAI-compatible Chat SSE source | 只处理显式注入的 Provider；上游错误、取消、超时和缺失终止事件必须分类 |
| `src-tauri/src/cursor/provider_factory.rs` | 只接受显式 `openai_chat`、URL 和 Bearer/ClaudeAuth 输入策略校验及 source 构造 | 已完成通用 factory、只读 `from_current` current-provider 解析和由 Provider source 提供的无请求体 preflight；不会解释 `openrouter_compat_mode` 或清理设备级 settings；database-backed Harness 已调用该 factory；启动前/请求后的 Provider 健康错误可联动脱敏状态，故障转移仍待完成 |
| `src-tauri/tests/cursor_provider.rs` | Provider stream、错误/取消和 factory 拒绝条件测试 | 使用合成 endpoint/凭据，不含真实 token、Cookie 或用户内容 |
| `docs/cursor/p2-contract.md` | 字段保留/丢失和延期项的机器可读前置说明 | P2-0/P2-1/P2-2 基础契约、P2-3a 只读 current-provider 绑定、P2-3b presence-aware 检测/前置拒绝和 P2-3c 启动前 preflight/失败回滚已落地；历史 hydrate、图片、MCP、完整工具生命周期和副作用边界仍延期 |

#### 3.1.2 P2 最小转换契约

P2 首个可接受实现只需覆盖以下边界：

1. 输入：`request_id`、`append_seqno`、Connect envelope、`AgentClientMessage`，以及可选父级请求头。
2. 路由：首条 `RunRequest` 的模型 ID 决定 Local/Upstream；后续消息只能沿既有 `request_id` 路由。
3. 上下文：保留会话 ID、当前用户消息、可验证的历史消息计数、模型参数和取消信号；图片和 MCP 只有在 blob/schema 校验通过时才进入 Provider。
4. 输出：至少支持文本增量、思考增量、工具开始、heartbeat、turn 结束、Connect 正常终止和结构化错误终止。
5. 明确缺口：工具参数增量、未捕获 ConversationAction、完整 Exec 工具结果、官方账号/会员接口不在首个 adapter 的完成定义内；真实验收关注自定义 endpoint/key，不要求官方账号登录。

任何不能保留的字段都必须在 adapter 的结果中标注 `dropped`/`unsupported`，不得静默吞掉；任何有副作用的工具或计划动作都不得按普通幂等请求自动重试。

### 3.2 `go-mitmproxy`：概念和测试参考

| 文件 | 行数 | 读取目的 | 不迁移内容 |
|---|---:|---|---|
| `proxy/entry.go` | 435 | 显式 HTTP 代理、CONNECT 接入和上游连接 | 不把 Go 包作为 Rust 依赖 |
| `proxy/attacker.go` | 691 | 动态叶子证书、TLS、H2、SSE/WebSocket 钩子 | 不直接复制其连接生命周期 |
| `cert/self_sign_ca.go` | 364 | CA 接口和证书材料管理 | 不使用其默认文件布局作为产品协议 |
| `proxy/addon.go`、`examples/*` | 见仓库 | addon 分层和测试顺序 | 不引入通用改包能力到默认 Cursor 路径 |

实现侧优先沿用 Rust 生态的 `hudsucker` + `rcgen`。只有明确决定使用 Go sidecar 时，才重新评估整个进程边界。

#### 3.2.1 MITM、流式与证书行号锚点

以下锚点均来自冻结 commit `3f5b4f8ee19d58784c6332ec946f46d3c2b2e73b`：

| 文件与行号 | 已证实行为 | 对 `cc2cx` 的迁移结论 |
|---|---|---|
| `proxy/entry.go:179-218` | 普通 HTTP 代理请求与 CONNECT 分开处理；非绝对 URL 被拒绝 | 保留显式代理入口和错误分类，不将代理入口当作业务 API |
| `proxy/entry.go:220-253` | CONNECT 按 `shouldIntercept` 选择直通或拦截，并区分 upstream cert 与 lazy intercept | Cursor 只允许按 host/path 白名单拦截，其他域必须直通 |
| `proxy/entry.go:255-309` | Hijack 后写入 `200 Connection Established`，直通连接用双向 transfer | 可作为连接级测试参考；Rust 实现仍由 `hudsucker` 管理连接生命周期 |
| `proxy/entry.go:312-370` | 首次攻击路径先窥探 TLS/WebSocket，再选择解密或原样传输 | 协议识别必须先于改写，未知流不能强行当作 HTTP |
| `proxy/entry.go:374-435` | lazy 路径的 TLS、WebSocket、普通 TCP 分支分别处理 | “支持 MITM”不等于所有连接都可解密，需保留旁路分支 |
| `proxy/attacker.go:47-85` | 初始化上游 `http.Transport`、可选上游代理、`ForceAttemptHTTP2`、TLS key log 和 H2 server | H2/诊断必须显式配置；key log 不得进入产品默认路径 |
| `proxy/attacker.go:102-132` | 客户端协商到 `h2` 时使用 `http2.Server` 与 `http2.Transport` | H2 只在协商成功的分支成立，不能宣称全链路恒定 H2 |
| `proxy/attacker.go:226-282` | 上游 TLS 复用客户端 SNI/ALPN/版本，并为服务端连接配置 H2 transport | 上下游 ALPN 需分别验证，不能仅凭客户端协商推断上游协议 |
| `proxy/attacker.go:408-438` | lazy HTTPS 明确只协商 `http/1.1` | H1.1 是必要回退，不应被 H2 优化强行替换 |
| `proxy/attacker.go:441-493` | 响应按 32 KiB 读取，每次写入后 `Flush` | 流式适配应保持低缓冲和 flush，但要单独处理背压与取消 |
| `proxy/sse.go:35-67,70-116` | SSE 增量读取，按 `\\n\\n` 切事件，仅对有 data 的事件触发 addon | 可借鉴事件边界测试；不能把 SSE 文本解析当作 Cursor protobuf 解析 |
| `proxy/sse.go:119-204` | EOF 时 flush 最后不完整事件并触发 SSEEnd，事件支持 data/event/id/retry | 结束和半帧必须纳入回归测试；事件数据仍应避免进入普通日志 |
| `proxy/websocket.go:90-152` | 先读客户端握手，再用已有连接连接上游，随后升级客户端 | WSS 旁路可复用双连接顺序，但不得默认启用通用改包 |
| `proxy/websocket.go:155-228` | 客户端到服务器、服务器到客户端各自 goroutine 转发；任一方向关闭会发送 Close | 双向流关闭语义可作为测试参考，不能替代 Cursor BidiAppend 的 request_id 状态机 |
| `cert/self_sign_ca.go:43-69` | 默认 RSA 2048，自签 CA 保存较宽的 ExtKeyUsage | 产品 CA 应缩小用途、独立命名和权限，并由平台安装动作控制信任 |
| `cert/self_sign_ca.go:100-163,179-234` | CA 默认持久化到用户目录 `.mitmproxy`，可从 PEM 恢复私钥和证书 | 不复制默认路径；私钥存储、权限和恢复策略必须由 cc2cx 明确定义 |
| `cert/self_sign_ca.go:305-328` | 叶子证书按 common name LRU 缓存，并用 singleflight 防并发重复生成 | 可借鉴并发生成去重，但要加入精确 host、有效期和缓存失效策略 |
| `proxy/proxy.go:16-24,46-80,99-137` | `SslInsecure`、日志路径、上游代理和 addon 都是全局能力 | `SslInsecure` 默认关闭；Cursor 路径不开放任意 addon 或完整 Flow 读写 |

明确风险：该项目存在 `SslInsecure` 上游证书校验绕过入口（`proxy/attacker.go:61-64,231-234`），并默认配置 TLS key log writer；其 Flow/addon 可访问完整请求、响应、SSE 和 WebSocket 内容。因此它只能作为连接生命周期和流式测试参考，不能作为 `cc2cx` 的默认 MITM 功能底座。

### 3.3 `cursor-fake`：边缘旁路参考

| 文件 | 行数 | 能力 | 边界 |
|---|---:|---|---|
| `main.go` | 779 | DNS fake-IP、真实 IP 映射、TCP 双向透传、可选远端 CONNECT | 不解密 TLS，不解析 HTTP，不改写 Cursor protobuf |

它只能作为显式代理不可用时的独立实验工具。不得把它作为 P1 的 Cursor 主接入。

#### 3.3.1 fake-IP 旁路行号锚点

以下锚点均来自冻结 commit `4d93f83fbd581e4c7632f527eb8b0b9fb4361943`：

| 文件与行号 | 已证实行为 | 对 `cc2cx` 的迁移结论 |
|---|---|---|
| `main.go:24-27` | fake-IP 基于 `127.0.0.x`，合成 A TTL 为 60 秒 | 这是系统 DNS/路由实验前提，不是 Cursor 协议能力 |
| `main.go:92-129` | 域名到 fake IP、fake IP 到 real IP 列表分别存储；地址分配到 255 后回绕，未见过期回收 | 若未来评估旁路，必须补 TTL、容量、冲突和回收策略 |
| `main.go:132-177` | 解析结果去重后写入映射；连接时从 real IP 列表随机选一个 | 随机选路不等于按延迟、失败率或负载选路，不能作为 P3 选路算法 |
| `main.go:399-426` | 仅劫持 `cursor.sh`、`cursor.com`、`cursorapi.com` 的 A/AAAA/ANY 查询，其余 DNS 透传 | 域名后缀匹配要有边界测试，不能扩大到所有海外域名 |
| `main.go:429-463` | 上游答案可提取 A/AAAA，但 `buildSyntheticAReply` 最终只生成 A 记录 | AAAA 查询存在语义缺口，IPv6 客户端不能据此认为可用 |
| `main.go:471-484` | 预热解析先试 A，再试 AAAA，单次解析超时 6 秒 | 预热只适合实验和启动诊断，不是实时健康或首 token 指标 |
| `main.go:487-502` | fake 响应固定为权威 A 记录，TTL 使用 `defaultTTL` | 系统 DNS 缓存、hosts 和本地路由会成为隐含代偿约束 |
| `main.go:637-655` | 多个 DNS resolver 按顺序尝试，仅在错误时回退 | 这是串行故障回退，不是并行竞速或质量评分 |
| `main.go:659-670` | 默认监听 `127.0.0.1:53` 和 `:443`，通常需要管理员/root 权限 | 不适合作为桌面产品默认启动路径，需显式权限和恢复设计 |
| `main.go:213-270` | forwarder 根据连接 `LocalAddr()` 查 fake 映射，可经远端 HTTP CONNECT，双向 `io.Copy` | TLS/HTTP/2 内容不解密、不理解、不改写；只能做透明旁路 |
| `main.go:749-766,774-778` | `-defaults` 预热内置域名后启动 forwarder 和 DNS server | 预热列表、监听和缓存生命周期都要独立于主 Cursor harness |

`cursor-fake` 的正确定位是“系统 DNS + TCP 透明转发实验”。它没有 CA、TLS 解密、Connect/protobuf、`request_id`、Provider 事件或取消语义；即使它能改善某些拨号路径，也不能独自实现 Cursor 本地化或 BYOK 适配。

### 3.4 Pilot 逆向资料：行为证据

| 资料 | 用途 | 证据级别 |
|---|---|---|
| `AICodings_Pilot_静态深挖_完整总结.md` | LocalProxy、CA、hosts、流式和选路行为摘要 | 静态行为证据 |
| `_re_out/work/aicodings-pilot-re/report/DEEP_ANALYSIS.md` | 端点、健康探测和模块拓扑 | 静态/运行日志证据 |
| `_re_out/work/aicodings-pilot-re/report/DEEP_ANALYSIS_02.md` | hosts、CA、StartProxy、bidi 判定 | 静态 + 未完全启用的运行观察 |
| `_re_out/work/aicodings-pilot-re/report/DEEP_ANALYSIS_03.md` | 当前样本未启动 `StartProxy` 的限制 | 已验证运行观察 |
| `_re_out/work/aicodings-pilot-re/evidence/E-*.md` | 单项证据和重现命令 | 逐项标注，不得合并成源码事实 |

Pilot 资料可帮助识别失败模式和测试项，但不能替代源码、运行时日志或可重复的行为证据。

## 4. 证据标签规则

- `已证实`：当前源码可直接定位，或命令可重复得到同一结果。
- `行为证据`：日志、符号、逆向产物显示该行为，但没有完整源码实现。
- `合理推断`：由多个证据推导出的设计假设，必须进入待验证清单。
- `待验证`：需要 mock、fixture、抓包或真实 Cursor 手工操作确认。

当前高置信结论：

1. `cc2cx` 的现有 HTTP 代理是 HTTP/1.1 JSON/Provider 网关。
2. 冻结 P0 审计时，`cc2cx` 尚无 Cursor 专用 CA/MITM/Connect/protobuf 接入模块；截至当前阶段，P1 模块、P2 首片协议/Provider 探针和 database-backed Harness/MITM backend 生命周期已落地。Provider 启动前/请求后的健康错误已可通过脱敏状态、`/health=503` 与 Harness `degraded` 观测；故障转移和真实 Cursor 闭环仍未完成。
3. `cursor-byok/local_app` 是最接近 `cc2cx` 技术栈的 Cursor 接入参考。
4. `go-mitmproxy` 和 `cursor-fake` 分别只覆盖通用 MITM 和不解密的 DNS/TCP 旁路。
5. Pilot 的 `/healthz`、H2 和 endpoint 证据不足以单独证明首 token 或端到端 H2 性能。

## 5. P0 后的合法输入（历史门槛）

只有在本文、`architecture.md`、`baseline.md` 和 `upstream-sync.md` 被复核后，才允许继续推进 P1/P2。P1 的合法新增边界是：

```text
cc2cx/src-tauri/src/cursor/{mod,ca,settings,routes,mitm,harness,error}.rs
```

P1 的新增内容应局限于已验证的 Cursor 接入边界；Agent runtime、系统网络入口、客户端集成和动态选路分别建立证据与验收条件后再推进。

以上是 P0/P1 入口的历史门槛，保留用于追溯；当前执行断点为真实 Cursor 自定义 endpoint/key 流程与故障转移绑定：历史、图片、MCP、完整工具生命周期和副作用边界尚未完成；真实 Cursor 接入仍待后续阶段。请求前/后的 Provider 健康错误分类和 Harness `degraded` 联动已完成。
