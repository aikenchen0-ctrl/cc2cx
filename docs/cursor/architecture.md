# Cursor 改造 P0 架构边界

生成日期：2026-09-04<br>
状态：P0 审计完成；P1 已落地设置事务、路由策略、CA 材料、Windows Root 信任检测、显式代理生命周期和 Tauri 命令。P2-0/P2-1 已完成 Connect/BidiAppend/RunSSE 首片 wire 探针、TransportRegistry 和合成流闭环；P2-2 已完成通用可取消 Provider bridge、注入式 OpenAI-compatible Chat SSE source 和严格 factory；P2-3a 已完成数据库 current-provider 到显式 factory 的绑定，database-backed Harness 已负责 backend/MITM 启停和失败回滚；P2-3c 已增加有界、无请求体启动前 Provider 探测和失败回滚；Provider 请求后的认证/超时/传输/协议/截断错误已可联动脱敏 `/health=503` 与 Harness `degraded`。故障转移绑定、真实 Cursor 验收和完整 Agent runtime 仍未完成。代理启动不会自动安装 CA；用户必须通过独立安装动作确认系统提示。macOS/Linux 仍使用显式安装命令。

## 1. 当前结构

```text
cc2cx/                                      # Tauri + React 产品壳
├─ src/                                     # React UI、设置页、Provider 表单、i18n
├─ src-tauri/src/lib.rs                     # Tauri 注册、AppState、启动恢复（2410 行）
├─ src-tauri/src/commands/                  # Tauri 命令，包括现有代理命令
├─ src-tauri/src/services/proxy.rs          # 现有 ProxyService（10003 行）
├─ src-tauri/src/proxy/                     # CLI/Provider HTTP 网关
│  ├─ server.rs                             # Hyper HTTP/1.1 accept loop（628 行）
│  ├─ handlers.rs                           # JSON/Provider handler（3581 行）
│  ├─ provider_router.rs                    # Provider 选择和现有故障转移
│  ├─ failover_switch.rs                    # Provider 级故障切换
│  └─ ...                                   # SSE、HTTP client、编码和用量
├─ src-tauri/src/settings.rs                # cc2cx 自身设置（1216 行）
└─ src-tauri/src/app_config.rs              # 应用类型和配置（1250 行）
```

当前主执行路径：

```text
Tauri command
  → ProxyService::start
  → ProxyServer::start
  → TcpListener::bind
  → hyper HTTP/1.1 connection
  → JSON/Provider handler
  → provider router / failover
  → upstream response
```

现有主路径已形成 database-backed Harness 的本机闭环，但尚未形成真实 Cursor 生产闭环：

- database-backed Harness 按 current provider 创建合成 backend listener，将实际地址交给 MITM，并在失败/停止时回滚；
- 现有故障转移策略绑定；启动前 Provider 探测、请求后的健康错误分类和 Harness `degraded` 联动已实现；
- Cursor CA 的 macOS/Linux 平台信任安装和检测；
- Cursor host/path 白名单与真实 Cursor 请求验收；
- Cursor Agent 工具、会话和 transport 语义。

## 2. P1 目标结构（已落地模块）

```text
cc2cx/src-tauri/src/
├─ proxy/                                   # 保持现有 CLI/Provider 网关
└─ cursor/                                  # 已建，Cursor 接入边界
   ├─ mod.rs                                # 模块导出
   ├─ ca.rs                                 # rcgen CA 材料、校验、平台安装命令与卸载
   ├─ settings.rs                           # Cursor settings JSONC 备份、写入、恢复
   ├─ routes.rs                             # host/path 白名单和透传策略
   ├─ mitm.rs                               # hudsucker 显式代理、CONNECT/TLS、流式转发
   ├─ harness.rs                            # 启停、状态、失败回滚
   └─ error.rs                              # Cursor 模块错误分类和 UI 映射
```

P1 的控制流：

```text
用户点击 Cursor 启动（Tauri 命令和前端入口已接入）
  → 读取并备份 Cursor settings.json
  → 检查/生成 CA
  → CA 未信任时保持 Degraded；用户单独执行安装动作并确认系统提示
  → 启动 127.0.0.1:<port> 显式代理
  → 写入 Cursor 用户代理设置
  → 重新探测 CA 信任状态并发布 Running/Degraded
  → CA 未信任时仍启动本机代理并发布 Degraded；信任后状态可变为 Running

Cursor
  → 用户级显式代理
  → host 判断：仅明确 Cursor host
  → CONNECT/TLS：仅目标 host 解密
  → path 判断：白名单本地化，其余透传/拒绝
  → mock 或可配置 backend

停止或启动失败
  → 停止监听并取消活动请求
  → 恢复 settings.json 原值（发现外部修改先报告）
  → 清理临时状态
  → 独立卸载命令仅在代理停止后删除精确 CA；停止集成本身不会删除信任
```

## 3. 所有权和状态边界

| 状态 | 所有者 | 可变更者 | 必须保证 |
|---|---|---|---|
| Cursor settings 备份 | `cursor/settings.rs` | Cursor harness | 原值可恢复，原子写入 |
| CA 材料 | `cursor/ca.rs` | CA 生命周期 | 私钥不进源码和普通日志 |
| 本机监听 | `cursor/mitm.rs` | Cursor harness | Stop 幂等，端口占用可解释 |
| host/path 策略 | `cursor/routes.rs` | 版本化配置 | 非目标域不误拦截 |
| Cursor 运行状态 | `cursor/harness.rs` | Tauri command | 启动失败反向回滚 |
| CLI/Provider 代理 | `proxy/`、`ProxyService` | 现有命令 | Cursor 失败不阻塞既有能力 |
| Cursor protobuf 语义 | P2 `cursor/protocol` 或 sidecar | P2 adapter | 不进入 JSON handler |
| 上游网络选路 | P3 独立 routing | P3 控制面 | 新会话选路，已建流保持粘性 |

## 4. P1 与现有 `proxy/` 的接口

P1 不修改现有 `proxy/handlers.rs` 的请求格式。Cursor adapter 只允许通过一个明确的 backend 接口交付请求：

```text
Cursor MITM
  → LocalRoute { host, path, method, headers, streaming_body }
  → mock/backend adapter
  → Cursor Protocol Adapter（P2 首片已实现）
  → cc2cx Provider Adapter（P2-2 source 与 P2-3a current-provider factory 已实现）
```

P2-2 已提供独立的 Provider 格式转换层和可取消 source。后续接入现有 Provider 或数据库配置时，仍必须明确：

- Cursor 模型 ID 到 Provider 模型 ID 的映射；
- Cursor 增量帧到 Provider SSE/stream 的映射；
- 工具调用、取消、错误和结束事件的映射；
- 哪些字段无法保留；
- 哪些请求可安全重试，哪些请求具有副作用。

禁止将 `application/connect+proto` body 直接交给 `proxy/handlers.rs`。

## 5. P2/P3 预留边界

```text
P2
├─ cursor/protocol/                    # Connect/BidiAppend 编解码和 Cursor request 生命周期
├─ cursor/backend_adapter/             # Cursor → Provider 或 sidecar
├─ cursor/adapter.rs                    # P2-0 内部四契约和事件映射（已新增）
├─ cursor/provider.rs                   # 可取消 CursorProvider 与 OpenAI-compatible Chat SSE source
├─ cursor/provider_factory.rs           # 严格的 Provider 配置/认证校验与 source 构造
├─ cursor/protocol_backend.rs           # loopback 合成 listener；standalone 显式启动，database-backed Harness 按 current provider 启动
└─ fixtures/                           # 脱敏协议样本和错误样本

P3
├─ routing/                            # endpoint 清单、探测、熔断、粘性
├─ telemetry/                          # DNS/TCP/TLS/TTFB/TTFT/断流指标
└─ diagnostics/                        # 用户可见的协议和回退原因

P4（独立立项）
├─ cursor/dns_bypass/                  # cursor-fake 风格旁路，默认关闭
├─ cursor/mirror/                      # Mirror/CDP 控制面
└─ cursor/patch/                       # 只有合法、可审计且用户明确授权时评估
```

## 6. 网络路径设计约束

```text
直连（若实测最佳）
  → 区域流式中继
  → 备用区域中继
  → H1.1 流式回退
```

- CDN 默认只承担 manifest、下载、静态资源和健康探测。
- 动态 Cursor 流只有在确认支持长连接、双向流、取消和低缓冲后才可使用 CDN。
- Sealos/NodePort 只能作为候选节点，不能作为唯一生产入口。
- 本机 MITM 默认 H1.1；H2 按本机到入口、入口到上游分别验证。
- `/healthz` 是廉价筛选，不是首 token 性能代理指标。
- 流建立后保持入口粘性；已输出或有工具副作用的请求不得盲目跨入口重放。
- TransportRegistry 对 request_id 保持 generation 粘性；旧 generation 的终止清理不能删除替代 transport，generation 耗尽时显式失败。

## 7. P0 决策记录

| 决策 | 结论 | 理由 |
|---|---|---|
| 产品主仓 | `cc2cx` | 已有产品壳、Provider、配置、故障转移和发布链路 |
| Cursor 接入参考 | `cursor-byok/local_app` | Rust/Tauri 同栈，已有 CA/MITM/settings/harness 分层 |
| MITM 实现 | `hudsucker` + `rcgen` | 与主仓同栈；Go 库只作概念参考 |
| Cursor 协议/Provider bridge | P2-0/P2-1 wire subset、P2-2 通用可取消 bridge、OpenAI-compatible source、严格 factory 和 P2-3a current-provider 绑定已做 | 当前仍是最小单轮适配；故障转移、完整 protobuf/双向 Agent 语义不能由 JSON 网关替代 |
| DNS fake | 默认关闭 | 不解密 TLS，无法承担 Cursor 本地协议改写 |
| HTTP/2 | 按链路验证 | 可能有复用收益，也可能引入中间盒缓冲和 bidi 失败 |
| 商业 Pilot 能力 | 不复制 | 缺乏公开契约，且会引入凭证、会员和供应链风险 |

## 8. 测试、日志与提交的数据边界

真实 token、Cookie、用户提示词、工作区文件路径、图片、API key 和 CA 私钥可以在获得授权的本机运行态验收中临时使用，但不得进入以下任何持久化或可扩散产物：

- 协议 fixture、录制包和快照；
- 普通日志、错误报告、截图和诊断导出；
- `docs/`、Issue、PR 描述和提交内容。

fixture 只保留合成 ID、合成模型名、帧长度、序号、状态码和错误类别。日志只记录 host、path、阶段耗时、协议、状态码和脱敏后的错误类别；不得记录完整请求体、`Authorization`、Cookie 或 CA 私钥。真实验收如需保留证据，只保存本机受权限保护的临时文件，并在测试结束后删除；文档只记录结论、条件、时间窗口和可复现命令。

## 9. P2 源码证据驱动的适配边界

`cursor-byok` 的协议实现说明 P2 不是把 protobuf 交给现有 JSON 网关，而是增加一个独立的事件适配边界：

```text
Connect envelope
  → BidiAppend 解码（request_id + append_seqno + AgentClientMessage）
  → 首条模型路由（Local / Upstream）
  → 会话与取消状态
  → ProviderStream<ModelEvent>
  → AgentServerMessage / Connect END_STREAM
  → RunSSE 重放订阅
```

当前合成 backend listener 的边界：它暴露 `/health`、`BidiAppend` 和 `RunSSE`，standalone 调用方显式传入 loopback 地址；database-backed Harness 会按 current provider 执行有界、无请求体 Provider 启动前探测，通过后创建 listener 并把实际地址交给 MITM，失败则不写入 proxy/settings。已具备 Connect framing、RunSSE 先到时等待、正常 `END_STREAM` 与客户端断开区分、断开取消传播，以及显式注入 `CursorProvider` 的测试闭环。Provider 流观测到认证、超时、传输或协议失败时，`/health` 返回 `503` 并暴露脱敏错误代码，Harness 状态变为 `degraded`；MITM→backend 仅保留自有协议头 allowlist。无数据库的 `CursorHarness::new()` 仍可保持 `backend = None`；这只是测试/standalone 默认值，不代表 AppState 的 database-backed 路径不启动 backend。当前仍未实现故障转移和真实 Cursor 验收；不能因为 listener 已能接受 TCP 就宣称 Cursor 已支持。

已确认的最小行为：

- `RunSSE` 与 `BidiAppend` 必须分开处理；`RunSSE` 按 `request_id` 等待已有路由并重放历史帧。
- 首条 `BidiAppend` 的模型 ID进入当前 provider 适配边界；模型别名/未知模型策略仍待实现，后续 append 只能沿同一 `request_id` 继续。
- `append_seqno`、会话 ID、父级请求头和心跳是协议状态，不是可选日志字段。
- Provider 事件至少要映射文本增量、思考增量、工具开始、turn 结束、heartbeat 和结构化结束错误。
- Cancel、InjectContext、CancelSubagent 必须进入运行时和工具生命周期；关闭 HTTP 连接不等价于完成取消。

首个 P2 adapter 尚未覆盖完整 Exec 工具结果、未捕获 ConversationAction、账号/会员接口、Cursor 官方账号登录和网络选路。真实验收目标是 Cursor 通过 cc2cx 使用自定义 endpoint/key；缺失字段必须显式标记为不支持，不能静默丢弃或把有副作用动作当作幂等请求重试。

`go-mitmproxy` 仅贡献连接级测试参考：CONNECT 的直通/拦截分支、TLS SNI 叶子证书、H1.1 回退、H2 协商分支、SSE flush 和 WebSocket 双向关闭。其 `SslInsecure`、TLS key log、宽泛 addon/Flow 内容访问和默认 `.mitmproxy` 私钥布局均不进入产品默认路径。

`cursor-fake` 仅保留为 P4 旁路实验候选：它通过系统 DNS 返回 `127.0.0.x` fake-IP，再用 TCP forwarder 透明连接真实 IP；不具备 TLS 解密、Cursor protobuf、Provider 或取消语义。其默认 `:53`/`:443` 监听、无 TTL 回收、AAAA 合成缺口和随机 real-IP 选择，使其不能成为桌面产品的默认网络路径或 P3 选路实现。

## 10. P0 退出条件

全部满足后才能进入 P1：

- [x] 主仓和参考仓 commit 已锁定。
- [x] `cc2cx` 现有代理入口和 HTTP/1.1 边界已定位。
- [x] 新 `cursor/` 与现有 `proxy/` 的所有权已分开。
- [x] 参考项目的可移植部分与禁止部分已列明。
- [x] P1/P2/P3/P4 顺序和回退条件已写明。
- [x] 测试工具缺失和未验证项已写入基线报告。
