# cc2cx Cursor 接入交接文档

更新时间：2026-09-08

## 1. 主仓与关键路径

主仓：

```text
C:\codeDev\cyberWork\aiapiexe\cc2cx
```

核心代码：

```text
src-tauri/src/cursor/
├── ca.rs                 CA 生成、当前用户证书安装/卸载、状态检测
├── harness.rs            Cursor 接入生命周期、backend、代理和 settings 协调
├── mitm.rs               Cursor 域名显式 HTTP/HTTPS MITM 与请求改写
├── protocol_backend.rs   本地 Cursor Connect/HTTP backend
├── protocol/             protobuf、Connect、Bidi、RunSSE 编解码
├── adapter.rs             Cursor 请求归一化、Provider 流桥接
├── provider.rs            Cursor Provider 抽象、模型路由、流式响应
├── provider_factory.rs    从数据库 current Provider 创建 Cursor Provider
├── transport.rs           request_id 传输、输出缓存和取消
├── routes.rs              Cursor 主机和路径白名单
├── settings.rs            Cursor settings 备份、写入和恢复
└── profile.rs             Cursor 路径发现、隔离 profile 启动
```

命令入口：

```text
src-tauri/src/commands/cursor.rs
src-tauri/src/lib.rs             注册 Tauri Cursor 命令
src-tauri/src/store.rs           创建默认 Cursor Harness
```

前端入口：

```text
src/components/settings/CursorIntegrationPanel.tsx
src/components/settings/ProxyTabContent.tsx
src/lib/api/settings.ts
src/types.ts
```

测试：

```text
src-tauri/tests/cursor_ca.rs
src-tauri/tests/cursor_adapter.rs
src-tauri/tests/cursor_backend.rs
src-tauri/tests/cursor_commands.rs
src-tauri/tests/cursor_protocol.rs
src-tauri/tests/cursor_provider.rs
src-tauri/tests/cursor_routes.rs
src-tauri/tests/cursor_settings.rs
src-tauri/tests/cursor_transport.rs
```

真实 Cursor runner：

```text
src-tauri/examples/cursor_e2e_lab.rs
```

## 2. 已实现能力

Agent 安装面板当前管理 12 个 Agent（含 DeepSeek Harness）。

- CA 材料生成、证书配对校验和当前用户 Root 安装/卸载。
- Cursor settings 事务备份、代理写入、外部修改检测和恢复。
- 显式本机 HTTP/HTTPS MITM，默认只处理 Cursor 域名。
- Cursor Connect/protobuf、BidiAppend、RunSSE 和最小 Agent Run 支持。
- gzip 请求、跨 body chunk、客户端 heartbeat、取消和请求半关闭处理。
- Provider bridge、模型路由、启动前探测、健康状态和错误分类。
- 数据库 current Provider 解析；Claude Provider 默认走 Anthropic Messages，OpenAI-compatible Provider 走 Chat Completions。
- 本地 `IsConnected`、`AvailableModels`、`GetUsableModels`、`GetServerConfig` 最小响应。
- 隔离 profile 和真实 Cursor E2E runner，避免修改默认 Cursor profile。
- `profile::launch_with_proxy` 支持隔离测试时注入代理环境变量、`--proxy-server` 和 `--disable-quic`。
- Agent 安装面板现已纳入 DeepSeek Harness：固定安装 `@deepseek-ai/dsh@0.1.1-rc.2`，并在 `cc2cx` ACP profile 注册同版本 `@deepseek-ai/dsh-acp`；Node.js/npm 依赖、镜像回退、进度事件和安装后校验复用既有链路。
- DeepSeek Harness 的 API Key、账号登录和 `.dsh` 凭据仍由用户自行配置，安装器不写入或输出任何凭据。

## 3. 当前真实测试结论

### 已确认

- 本机 Cursor 可执行文件：

```text
C:\Program Files\cursor\Cursor.exe
```

- cc2cx Debug 可执行文件：

```text
C:\codeDev\cyberWork\aiapiexe\cc2cx\src-tauri\target\debug\cc-launch.exe
```

- E2E runner 可执行文件：

```text
C:\codeDev\cyberWork\aiapiexe\cc2cx\src-tauri\target\debug\examples\cursor_e2e_lab.exe
```

- 代理和本地 backend 能正常监听，backend `/health` 返回 `protocol=cursor-connect-subset-v1`，Provider 状态为 configured。
- 默认 Cursor 和隔离 Cursor 都能通过本机显式代理发出 `api2.cursor.sh` / `api3.cursor.sh` 请求。
- `Auto` 模式可以在 Cursor 中成功回复固定消息。

### 尚未证明

- 当前 cc2cx 日志没有出现 `AgentService/Run`、`RunSSE` 或 `BidiAppend`。
- Cursor 的 NodeService 仍存在直接连接外部 `443` 的进程连接。
- Cursor 结构化日志显示命名模型在网络请求前被 Free 计划门禁拒绝：

```text
model_intent = gpt-5.6-sol
bring_your_own_key = false
outcome = error
error_code = upgrade
pre_network
```

- 因此当前成功回复来自 Cursor 官方服务，不能作为自定义 endpoint/key 已接管的证据。
- 尚未完成 Windows DNS/fake-IP/TCP 透明入口与现有 MITM/backend 的组合验收。

## 4. 设计与计划文件

已提交设计：

```text
docs/superpowers/specs/2026-09-08-cursor-transparent-entry-design.md
```

已提交实现计划：

```text
docs/superpowers/plans/2026-09-08-cursor-transparent-entry.md
```

计划提交：`403af24`

计划中的实现顺序：

1. Hosts 可回滚事务。
2. fake-IP 映射。
3. 每个 fake-IP 的透明 TLS listener。
4. 解密后的 HTTP/2/Connect 接入现有 backend。
5. Harness 生命周期和回滚。
6. 隔离真实 Cursor E2E。
7. 文档、静态检查和完整回归。

## 5. 参考项目及可迁移边界

参考源码：

```text
C:\codeDev\cyberWork\aiapiexe\cursor-byok
C:\codeDev\cyberWork\aiapiexe\cursor-fake
C:\codeDev\cyberWork\aiapiexe\go-mitmproxy
C:\codeDev\cyberWork\aiapiexe\aicodings-pilot
```

可迁移：

- `cursor-byok` 的 Cursor protobuf、Transport、Conversation、Provider 和本地代理路由。
- `cursor-fake` 的 DNS/fake-IP 映射、TCP 入口和远端拨号结构。
- `go-mitmproxy` 的 CONNECT、动态证书、TLS 和 HTTP/2 处理。
- AICodings 的 Hosts/强制路由、HTTP/2 兼容、帧级诊断、延迟选路和连接证据。


## 6. 本地启动与验证

启动前端 renderer：

```powershell
cd C:\codeDev\cyberWork\aiapiexe\cc2cx
pnpm run dev:renderer
```

构建 Debug：

```powershell
$env:CARGO_BUILD_JOBS='1'
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin cc-launch
```

启动 Debug：

```powershell
& C:\codeDev\cyberWork\aiapiexe\cc2cx\src-tauri\target\debug\cc-launch.exe
```

构建 E2E runner：

```powershell
$env:CARGO_BUILD_JOBS='1'
cargo build --locked --manifest-path src-tauri/Cargo.toml --example cursor_e2e_lab
```

启动隔离 E2E：

```powershell
$env:CC2CX_E2E_CA_DIR='C:\Users\血饮\.cc-launch\cursor-ca'
$env:CC2CX_E2E_SKIP_CA_CHECK='1'
& C:\codeDev\cyberWork\aiapiexe\cc2cx\src-tauri\target\debug\examples\cursor_e2e_lab.exe
```

runner 使用：

```text
profile: C:\Users\血饮\.cc-launch\cursor-e2e-profile
CA:     C:\Users\血饮\.cc-launch\cursor-ca
```

打开 runner 启动的隔离 Cursor 后发送：

```text
CC2CX_E2E_OK
```

确认请求是否进入本地链路时，至少检查：

```powershell
Get-NetTCPConnection -State Established
Get-Content C:\Users\血饮\.cc-launch\logs\cc-launch.log -Tail 300
```

不得把日志中的 Authorization、Cookie、token、key 或完整提示词写入报告。

## 7. 接手后的第一步

1. 阅读透明入口设计和实现计划。
2. 确认所有 Cursor 进程已关闭，避免单实例转发造成假测试。
3. 先实现 Hosts 事务和 fake-IP 映射，并为每个模块建立失败测试。
4. 再实现每个 fake-IP `:443` listener；不要把 fake-IP 连接错误地绑定到单一 `127.0.0.1:443`。
5. 保持现有 `CursorRelay`、protocol backend 和 Provider 为唯一业务路径。
6. 只有看到本地 backend 的 Agent 请求和 Provider endpoint 证据后，才能宣称真实 Cursor 接入完成。

## 8. Evidence → Finding → Path

| Evidence | Finding | Path |
|---|---|---|
| Debug 构建产物存在，Cursor Harness/Provider 测试文件齐全 | 协议、Provider 和 Harness 底座已落地 | `src-tauri/src/cursor/`、`src-tauri/tests/` |
| cc2cx backend `/health` 返回 configured | 本地 backend 可启动并解析 current Provider | `src-tauri/src/cursor/protocol_backend.rs`、`provider_factory.rs` |
| Cursor 日志记录 `pre_network` + `error_code=upgrade` | 命名模型被官方 Free 计划门禁拦截，非 Provider 请求失败 | `C:\Users\血饮\.cc-launch\cursor-e2e-profile\logs\...` |
| Cursor NodeService 存在外部 `443` 连接，cc2cx 无 AgentService/Run 记录 | 显式 HTTP proxy 尚未覆盖真实 AI 通道 | `src-tauri/src/cursor/mitm.rs`、待实现 `transparent.rs` |
| 参考项目分别提供协议、MITM、DNS/TCP 和强制路由实现 | 下一步必须补透明入口，而不是继续修改模型列表 | `cursor-byok/server/src/local_app/proxy.rs`、`cursor-fake/main.go`、AICodings 静态总结 |

## 9. 工作树和提交纪律

当前主仓存在大量未提交的既有改动。接手时：

- 不要重置、清理或覆盖无关改动。
- 设计文档提交为 `a6e9743`，实现计划提交为 `403af24`。
- 后续实现按计划分任务提交，避免把无关前端、依赖升级或旧功能改动混入透明入口提交。

### Agent 安装扩展（2026-09-13）

DeepSeek Harness 相关实现集中在：

```text
src-tauri/src/commands/misc.rs
src/components/agent-install/AgentInstallPanel.tsx
tests/components/AgentInstallPanel.test.tsx
docs/superpowers/specs/2026-09-13-deepseek-harness-install-design.md
docs/superpowers/plans/2026-09-13-deepseek-harness-install.md
```

当前已完成代码级接入、专项测试和真实 npm 安装；尚未执行需要 API Key 的模型对话。真实使用时由用户单独配置 DeepSeek API Key。

### 旧 CC Switch 一键迁移（2026-09-13）

安装 Agent 页面检测到 `~/.cc-switch/cc-switch.db` 时，按钮会直接调用 `migrate_legacy_cc_switch`：后端只读快照旧数据库，复用 staging/schema migration/安全备份/恢复锁完成导入。自动迁移失败时仍可从 SQL 导出文件手动导入；旧数据库始终不写入或删除。
