# Cursor 改造 P0 基线与验证报告

生成日期：2026-09-07<br>
范围：P0 审计、P1 的可逆本机接入底座，以及 P2 协议首片、Provider 流桥接、显式 OpenAI-compatible Chat source、严格 Provider factory 和数据库 current-provider 绑定。CA、MITM、harness、Connect/BidiAppend/RunSSE wire 解码边界、最小合成 HTTP backend、仅 loopback 的显式 listener 探针、可取消 Provider bridge、注入式 SSE source、启动前无请求体 Provider 探测和基于现有 ProviderAdapter 的 factory 已实现并通过专项测试；Windows Root 信任库检测已接入，CA 安装改为独立显式动作。database-backed Harness 已能按 current provider 管理 backend/MITM 启停、探测失败阻断和失败回滚；Provider 请求后的认证/超时/传输/协议/截断错误已可联动 `/health=503` 与 Harness `degraded`，故障转移绑定、macOS/Linux 探针和真实 Cursor 验收仍待完成。

## 1. 基线摘要

| 项目 | 结果 | 说明 |
|---|---|---|
| 主仓路径 | 通过 | `C:\codeDev\cyberWork\aiapiexe\cc2cx` 是 Git 根目录 |
| 主仓 commit | 通过 | `40b3bcd80fd787ba1f2c11197783409a76a1657a` |
| 参考 commit | 通过 | 见 `source-map.md` |
| 主仓工作树 | 有未提交改动 | P2 切片仍在工作树中；未修改参考仓。提交前必须按文件所有权审阅，不能把当前未提交状态当作稳定版本 |
| Cursor 模块 | 部分实现 | 已新增 `cursor/{error,ca,routes,settings,mitm,harness,profile,protocol,adapter,transport,protocol_backend,provider,provider_factory}.rs`；Connect/BidiAppend wire subset、最小 Run 双向首片、跨 body chunk 重组、请求 gzip 有界解压、后续 `CancelAction` 取消、客户端 heartbeat、请求半关闭、unsupported 后续帧显式错误、`IsConnected` 探针、`AvailableModels/GetUsableModels/GetDefaultModel/GetDefaultModelForCli` 本地模型目录、最小 `GetServerConfig` 本地安全默认、TransportRegistry、RunSSE framing/终止帧、合成/注入式 HTTP 闭环、显式 loopback listener 生命周期、OpenAI-compatible SSE source、严格 factory、启动前无请求体探测和 current-provider 绑定测试通过。database-backed Harness 已接线并把实际 backend 地址交给 MITM；隔离 profile 构造不会触碰默认 Cursor 用户目录；Provider 观测到的认证/超时/传输/协议错误可反映为 backend degraded，MITM 有自有 header allowlist；账户/会员控制面、旧式 StreamChat/StreamEdit/StreamReview、故障转移、macOS/Linux 探针和真实 Cursor 验收尚未完成 |
| 现有代理 | 已定位 | `ProxyService` → `ProxyServer` → Hyper HTTP/1.1 → JSON/Provider handler |
| Rust 测试 | P1/P2 专项通过 | P1 源码测试项为 CA 5、commands 10、routes 3、settings 7（合计 25）；本轮 P2 源码测试项为 protocol 18、adapter 15、backend 29、provider 31、transport 11（合计 104/104）；2026-09-08 单线程回归通过 Run gzip、跨 body chunk、CancelAction、客户端 heartbeat、请求半关闭、unsupported 后续帧、模型目录和 GetServerConfig fixture；完整工作区回归仍需单独执行 |
| Rust 格式检查 | 通过 | 使用 `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` 完成全仓格式检查，退出码为 0 |
| 前端单测 | 部分通过 | `141` 个测试文件中 `139` 通过、`2` 个既有测试超时（`PiProviderForm` 1 项、`App` 集成 1 项）；另有预期 MSW/CodeMirror stderr |
| P1/P2 编译 | P1/P2 专项编译并通过 | P1 的 `cursor_ca`、`cursor_commands`、`cursor_routes`、`cursor_settings` 与 P2 的 `cursor_protocol`、`cursor_adapter`、`cursor_backend`、`cursor_provider`、`cursor_transport`、`cursor::profile` 均已在 `--locked`、单线程测试下编译并通过；`Run` 后续取消/半关闭测试已加入 backend 回归；全仓回归和前端回归仍需单独执行 |
| Windows Root 证书库闭环 | 通过 | 独立临时根证书成功加入当前用户 `Root`、按指纹查询、删除并确认不再出现；测试目录已清理 |

## 2. 执行过的检查

### 2.1 Git 与版本

```text
git -C C:\codeDev\cyberWork\aiapiexe\cc2cx rev-parse HEAD
40b3bcd80fd787ba1f2c11197783409a76a1657a

git -C C:\codeDev\cyberWork\aiapiexe\cursor-byok rev-parse HEAD
2f3bffd15ac2b67b8808c3a2eda6abc5611b1b2a

git -C C:\codeDev\cyberWork\aiapiexe\go-mitmproxy rev-parse HEAD
3f5b4f8ee19d58784c6332ec946f46d3c2b2e73b

git -C C:\codeDev\cyberWork\aiapiexe\cursor-fake rev-parse HEAD
4d93f83fbd581e4c7632f527eb8b0b9fb4361943
```

参考仓工作树均无修改。主仓之外的工作区可能有其他项目变更，本轮未触碰。

### 2.2 主仓入口定位

已定位的现有入口：

```text
src-tauri/src/lib.rs:342       pub fn run()
src-tauri/src/lib.rs:1548     commands::start_proxy_server
src-tauri/src/lib.rs:1549     commands::stop_proxy_server
src-tauri/src/lib.rs:1550     commands::stop_proxy_with_restore
src-tauri/src/lib.rs:1553     commands::get_proxy_status
src-tauri/src/proxy/server.rs:54   pub struct ProxyServer
src-tauri/src/proxy/server.rs:94   ProxyServer::start
src-tauri/src/proxy/server.rs:225  ProxyServer::stop
src-tauri/src/services/proxy.rs:389  pub struct ProxyService
src-tauri/src/services/proxy.rs:932  ProxyService::start
src-tauri/src/services/proxy.rs:1714 ProxyService::stop
```

`ProxyServer::start` 使用 `tokio::net::TcpListener` 和 Hyper `http1::Builder`，未发现 Cursor CONNECT/TLS/MITM 前门。现有 `proxy/handlers.rs` 是 JSON/Provider 处理路径，不能直接承载 Cursor protobuf。

### 2.3 参考能力定位

### 2.4 P1 已落地的逻辑与运行时

```text
src-tauri/src/cursor/routes.rs
  - Cursor host 大小写/末尾点归一化
  - 明确 path 白名单
  - 非 Cursor host 永远透传
  - 未命中 path 的透传/拒绝策略

src-tauri/src/cursor/settings.rs
  - 用户级 settings.json(JSONC) 读取
  - 代理键写入和原子替换
  - 原值、SHA-256 和应用后 SHA-256 私有备份
  - 外部修改冲突检测
  - 应用前崩溃窗口的未提交备份清理

src-tauri/src/cursor/ca.rs
  - RSA 3072 CA 生成、持久化和证书/私钥校验
  - Windows 当前用户 Root 精确 DER 探测/显式安装与删除；macOS/Linux 仍提供显式命令

src-tauri/src/cursor/mitm.rs
  - 127.0.0.1 显式 HTTP(S) 代理
  - Cursor host 的 CONNECT/TLS 拦截和目标 path 分流
  - 解密后的 origin-form 请求通过 Host 头参与路由

src-tauri/src/cursor/harness.rs
  - CA 初始化、代理启停、settings 应用/恢复和状态查询
  - 启动失败回滚、停止幂等
```

这些模块只绑定本机显式代理、不改现有 `proxy/` JSON handler。Tauri 已注册状态、CA 初始化、安装、卸载、启动和停止命令；启动不会自动安装 CA，用户需单独执行安装动作并确认系统提示。Windows 按完整 DER 精确探测/删除，其他平台仍返回未信任，卸载要求代理已停止，真实 Cursor 验收尚未完成。

```text
cursor-byok/server/src/local_app/proxy.rs:152  is_cursor_host
cursor-byok/server/src/local_app/proxy.rs:157  is_local_path
cursor-byok/server/src/local_app/settings.rs:61 write_proxy_settings
cursor-byok/server/src/local_app/settings.rs:71 clear_proxy_settings
cursor-byok/server/src/api/cursor/bidi.rs:156   decode
cursor-byok/server/src/api/cursor/run_sse.rs:20 stream
go-mitmproxy/proxy/entry.go:196               CONNECT handling
go-mitmproxy/proxy/attacker.go:48             newCa
cursor-fake/main.go:214                      startForwarder
cursor-fake/main.go:399                      handleDNSQuery
```

## 3. 可用脚本入口

`cc2cx/package.json` 定义了：

```text
pnpm dev
pnpm build
pnpm build:renderer
pnpm typecheck
pnpm test:unit
pnpm format:check
```

本轮尝试：

```text
pnpm exec vitest run
→ 已执行：现有测试输出均通过；stderr 包含预期的 MSW/CodeMirror 警告，未取得独立退出摘要

cargo test --manifest-path src-tauri/Cargo.toml --test cursor_routes --test cursor_settings
→ 通过：routes 3/3、settings 7/7

cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_ca --test cursor_commands --test cursor_routes --test cursor_settings -- --test-threads=1
→ 本轮通过：CA 5/5、commands 10/10、routes 3/3、settings 7/7；Windows 条件编译实际运行 24/24（源码总项 25，非 Windows CA trust 场景被条件排除）

cargo test --manifest-path src-tauri/Cargo.toml --test cursor_protocol
→ 通过：协议探针 18/18

cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_adapter -- --test-threads=1
→ 本轮通过：Provider bridge 与内部契约 15/15

cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_backend -- --test-threads=1
→ 本轮通过：合成/注入式 backend、current-provider backend、Provider 健康错误分类、header allowlist、未实现字段前置拒绝、listener 和 RunSSE 断开传播 17/17

cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_provider -- --test-threads=1
→ 本轮通过：OpenAI-compatible SSE source、启动前无请求体 preflight、严格 Provider factory、只读 current-provider 绑定和模型路由 31/31

Windows 证书库独立闭环
→ 通过：`certutil -user -addstore -f Root` 写入、`certutil -user -store Root <thumbprint>` 查询、`certutil -user -delstore Root <thumbprint>` 删除；删除后按主题和指纹检索无残留
```

2026-09-07 本轮低并发专项回归已取得当前证据：P1 源码测试项 25 个，本机 Windows 实际运行 24/24；P2 五个测试文件共 92/92 通过（protocol 18、adapter 15、backend 17、provider 31、transport 11），commands 10/10。独立合成 backend listener 只绑定 loopback，database-backed Harness 会按 current provider 先执行无请求体启动前探测，成功后启动 listener 并把实际地址交给 MITM，探测失败不写入 proxy/settings 并释放资源。`run_provider_stream`、显式注入的 OpenAI-compatible source、`provider_factory`、只读 current-provider 解析、presence-aware 前置拒绝、模型路由、Provider 启动前/请求后健康错误分类和 backend header allowlist 均已覆盖；尚未接入现有故障转移、完整 Agent runtime 或真实 Cursor E2E。全工作区回归、真实 Cursor 验收和平台扩展仍需专门环境。当前锁文件包含约 695 行新增、128 行删除，存在较大无关依赖升级风险，应在提交前审阅。前端基线存在两个超时，需单独排查，不能归因于 Cursor 改造。

## 4. 现有基线风险

1. `src-tauri/src/lib.rs`、`services/proxy.rs` 和 `handlers.rs` 体量较大，Cursor 代码必须保持独立目录，避免继续膨胀热点文件；当前 bridge 仍保持在 `cursor/adapter.rs`，不得为了接 Provider 而直接改 JSON handler。
2. Cargo 已解析并编译 `hudsucker`、`rcgen`、`reqwest` 等 Cursor 依赖；后续升级 Cargo.lock 时仍需审阅无关依赖变更并重跑专项。
3. 主仓有现有代理启停和恢复逻辑，但它针对 CLI/Provider 接管，不自动覆盖 Cursor settings、CA 或本机 MITM。
4. 现有前端有代理状态和全局代理 UI；Cursor 状态应使用独立命名空间和现有 i18n，不复用会误导用户的 CLI 代理状态。
5. Pilot 的管理面选路日志不能证明 Cursor 主代理已经启动，也不能作为本仓性能基线。

## 5. 后续真实 Cursor/P3 必须补齐的验证环境

- `cargo --version`、`rustc --version`、`rustfmt --version`、`cargo clippy --version`。
- `node --version`、`pnpm --version`，并完成 `pnpm install --frozen-lockfile` 或等价的依赖恢复。
- Windows 测试用户的 Cursor 安装目录、用户 settings 备份目录和临时 CA 目录。
- 本机 mock backend，能返回增量流、慢流、断流、取消和错误。
- 不含真实密钥、Cookie、用户内容的协议 fixture。

## 6. 当前不宣称的内容

以下内容在本轮均为未实现或未验证：

- Cursor 可以通过 `cc2cx` 完成真实对话；
- CA 已在真实 Cursor 中成功使用（仅 Windows 证书库安装闭环已通过，Cursor 真实验收、macOS/Linux 尚未确认）；
- Cursor `settings.json` 已在真实用户目录可逆修改（仅临时目录事务测试已写入）；
- Tauri 命令启动后一定能被 Cursor 使用（协议适配和真实 Cursor 验收尚未完成）；
- `BidiAppend` 或 `RunSSE` 已可用于真实 Cursor（当前只有本机合成 backend，不是完整生产运行时）；
- CDN、Sealos 或 H2 已带来性能提升；
- 已实现按首 token 延迟选路；
- 已达到 AICodings Pilot 全功能。

## 7. P0/P1/P2 当前结论

P0 审计产物完整。P1 已实现可逆路由、设置事务、CA 材料、Windows Root 信任探针、独立显式安装/精确删除、显式 MITM 前门和 harness 命令。P2 首片已新增 `cursor/protocol`、`cursor/adapter.rs`、`cursor/transport.rs`、`cursor/protocol_backend.rs`、`cursor/provider.rs` 和 `cursor/provider_factory.rs`：前者提供 Connect 单帧编解码、`BidiAppendRequest` subset、`AgentClientMessage`/`AgentRunRequest` 的模型与会话提取，以及 RunSSE 请求 ID、数据帧和终止帧顺序探针；adapter 建立 `CursorRequest`、`CursorAction`、`ProviderInvocation`、`CursorOutputEvent` 四个内部契约和 presence-aware 字段保留/丢失矩阵，并提供 `run_provider_stream` 的取消、错误、截断和空闲边界；backend 对实际出现的历史、图片、MCP、reasoning 字段在 transport/provider 创建前返回显式 `NOT_IMPLEMENTED`，unsupported action 同样显式拒绝；provider 定义可取消 source，并以合成 OpenAI-compatible SSE fixture 验证请求构造、文本/usage/done、工具参数重组、错误和取消；factory 复用 cc2cx 现有 ProviderType/ProviderAdapter，只允许显式 `openai_chat`，不把 legacy `openrouter_compat_mode`、Gemini、Anthropic 原生、Responses、OAuth 或 Copilot 伪装成 Chat，并要求不带 userinfo 的有效 URL 和 Bearer/ClaudeAuth 输入策略（出站统一 Bearer）；模型路由先处理显式别名，再接受已声明上游模型，最后按显式默认模型回退，未知模型无默认值时明确拒绝；`from_current` 使用只读 effective current-provider 选择（有效设备级 settings ID 优先，stale 只回退数据库 `is_current` 且不改写 settings）并覆盖数据库缺失、不兼容、认证缺失、本地优先级和 stale settings 保留；transport/backend 将验证后的单轮请求接入可重放合成流或显式注入流，并提供 database-backed Harness/current-provider 构造；Provider preflight 使用共享 HTTP client 发无请求体 GET，8 秒上限，认证/5xx/超时/传输错误有脱敏分类，Harness 在 proxy/settings 写入前阻断失败并验证回滚；`CursorProtocolBackend::start()` 只提供显式 loopback listener，但 database-backed Harness 会调用它并把实际地址传给 MITM，具备绑定、幂等停止、失败回滚、客户端断开传播和端口释放测试。当前源码测试项为 P1 25、P2 92；2026-09-07 低并发专项回归中 P2 五个测试文件共 92/92 通过（protocol 18、adapter 15、backend 17、provider 31、transport 11），commands 10/10。Provider 启动前/请求后的认证、超时、传输、协议/截断错误可记录为脱敏健康代码，使启动失败阻断 settings/proxy，后续 `/health` 返回 `503` 并令 Harness 进入 `degraded`；故障转移绑定、全仓回归、真实 Cursor 验收仍未完成。全仓 `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` 已退出码 0；当前仍不能声称 Cursor 已支持：完整 Agent runtime、历史/图片/MCP/工具执行、真实 Cursor 对话/取消、macOS/Linux 信任探针，以及 CDN/H2/动态选路仍缺失。

## 8. 透明入口（2026-09-09，入口互斥修正）

目标：把**不走** `http.proxy` 的 Cursor AI 连接导入现有 protocol backend。这是模式 T。模式 P 是已有的显式 MITM（对齐 cursor-byok），另场验收。

### 两种入口不得同场

| 模式 | 参考 | 打开 | 禁止 |
|---|---|---|---|
| T 透明 | cursor-fake 的 fake-IP 形状 + 本机 TLS 终止 | Hosts 标记块、每主机 `:443`、HTTP/2 | `http.proxy`、`--proxy-server` |
| P 显式代理 | cursor-byok | `http.proxy`、`disableHttp2=true`、本机 MITM | 系统 Hosts、fake-IP `:443` |

同一次 `start()` / 同一隔离窗口里两套都开，不是增强，而是：

- Agent `Run` 被 `--proxy-server` 送进 MITM，透明 `:443` 上的连接不能当模式 T 证据。
- Hosts 把 `api2.cursor.sh` 指到 `127.0.0.2` 后，MITM 旁路官方地址会解析回 loopback。

2026-09-09 隔离 E2E 当时同时写了 Hosts、启动了 MITM、并用 `--proxy-server` 拉起 Cursor。日志里 `transportHost=api2.cursor.sh` 且 HTTP 408，属于模式 P 上的 Connect/gzip/流式问题，**不是**透明入口验收失败。

### Windows 前置（仅模式 T）

- 系统 Hosts：`%SystemRoot%\System32\drivers\etc\hosts`。
- 标记块 `# BEGIN CC2CX CURSOR` / `# END CC2CX CURSOR`，当前允许：`api2.cursor.sh`、`api3.cursor.sh`、`agentn.global.api5.cursor.sh` → `127.0.0.2` 起。
- 每个 fake-IP 绑自己的 `:443`。
- Hosts 不可写则返回「需要提升权限」，不改文件，并停掉已绑定的 listener。
- CA 仍是独立安装；透明入口用现有 CA 签叶证书。
- 单元测试用临时 Hosts 文件，不碰系统 Hosts。

### 回滚

按**当前启用的那一种模式**回滚，不要写成一条「Hosts + MITM + settings」流水线。

- 模式 T：停 listener → 恢复 Hosts。不要顺带写入 `http.proxy`。
- 模式 P：停 MITM/backend → 恢复 settings。不要写 Hosts。
- Hosts 或 settings 被外部修改时，拒绝覆盖。

`CursorHarness::start()` 一次只启用一种入口：默认模式 P（MITM / `http.proxy`，不写 Hosts）；`enable_transparent_entry` 后为模式 T（Hosts / fake-IP `:443`，不启动 MITM）。`cursor_e2e_lab` 只跑模式 T，用 `launch` 而不是 `launch_with_proxy`。验收时不得把历史双入口窗口当成模式 T 已通过。

### 支持的本机路径

白名单仍是 `/agent.v1.AgentService/Run`、`RunSSE`、`BidiAppend`，以及最小的 `IsConnected` / `AvailableModels` / `GetUsableModels` / `GetServerConfig`。未匹配路径 fail-closed。不转发 `Authorization`/`Cookie`。应转发 `content-encoding` / `connect-content-encoding`，否则 gzip Connect 体会被当成裸 protobuf。

### Auto 官方路由 vs cc2cx Provider

Cursor `Auto` 能回复，不能当作自定义 endpoint/key 已接管。宣称接管必须同时有：

1. 本机 backend 出现 `AgentService/Run`（或当前协议等价路径）。
2. Provider `/health` 不是 `not_observed`，且能对应到当前配置 endpoint。
3. 连接证据与所报模式一致：模式 T 看 fake-IP `:443`，模式 P 看 MITM 端口，且该场没有另一套入口。

### 本切片验证范围

专项测试覆盖 Hosts 事务、fake-IP、SNI/ALPN、合成 HTTP/2 转发、Harness 回滚、E2E 契约（隔离 profile、CA 门禁、脱敏元数据、入口互斥）。真实隔离窗口必须按所报模式验收：模式 T 无 `--proxy-server` / `http.proxy`；模式 P 无 CC2CX Hosts 块。历史双入口窗口不能写成透明入口已通过。
