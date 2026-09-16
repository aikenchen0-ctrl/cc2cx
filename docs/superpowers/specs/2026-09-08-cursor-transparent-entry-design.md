# Cursor 透明入口接管设计

状态：设计已批准；入口模式在 2026-09-09 按参考实现与真实窗口证据做了互斥修正

## 目标

透明入口只解决一类问题：Cursor 有些 AI 连接**不走** `http.proxy`。它不是显式 MITM 的加强版，也不能和显式代理叠在同一场验收里。

```text
模式 T（透明，本设计）
Cursor NodeService
  -> 受控 Hosts（无 http.proxy、无 --proxy-server）
  -> loopback fake-IP:443
  -> 透明 TLS 终止
  -> HTTP/2/Connect
  -> 现有 Cursor backend
  -> 当前数据库 Provider

模式 P（显式代理，cursor-byok 路径，已有 mitm.rs）
Cursor settings http.proxy + disableHttp2
  以及可选 --proxy-server
  -> 本机 hudsucker MITM
  -> 白名单改写到同一 backend
```

## 入口互斥（2026-09-09 修正）

参考：

- `cursor-byok` 只写 `http.proxy` / `disableHttp2` / `systemCertificatesV2`，**不改 Hosts**。
- `cursor-fake` 只做 DNS/fake-IP/TCP 透传，**不解密、不写 http.proxy**。
- `go-mitmproxy` 文档写明是显式代理，不是透明代理。

因此：

1. 证明模式 P：必须**不写**系统 Hosts。Hosts 会把 `api2.cursor.sh` 解析成 `127.0.0.2`，MITM 若旁路官方地址会连回自己。
2. 证明模式 T：必须**不写** `http.proxy`，也不得用 `--proxy-server` 启动 Cursor。否则 Agent `Run` 走 MITM，透明 `:443` 上的连接不能当作接管证据。
3. Harness 可以同时具备两套代码，但一次 `start()` / 一场 E2E 只能启用一种入口。把两套同时打开不是覆盖更全，是互相污染。
4. 2026-09-09 隔离窗口里 `transportHost=api2.cursor.sh` 且进程带 `--proxy-server`，复现的 HTTP 408 属于模式 P，不能写成模式 T 失败。

## 非目标

- 不接管非 Cursor 域名。
- 不把未知协议降级为任意 Provider 请求。
- 不在同一运行实例里用 Hosts 给 MITM 提供“官方上游 IP”。fake-IP 表只保存 hostname ↔ loopback，不做 cursor-fake 那种 fake→real 透传。

## 组件

### Hosts 管理器

- 仅维护 `CC2CX` 标记块。
- 每次启用前保存原始文件哈希和原文。
- 只写入明确允许的 Cursor 主机名。
- 停用时仅在文件仍匹配“原文 + CC2CX 块”的预期状态时恢复；发生外部修改则拒绝覆盖并报告冲突。
- 所有启停操作可重复执行，失败时恢复已完成的部分。

### Fake-IP 映射器

- 从受控地址池分配 loopback 地址，例如 `127.0.0.2` 起的连续范围。
- 只保存 `fake_ip ↔ hostname`。不保存、不拨官方上游 IP；模式 T 在本机终止 TLS，不透传到 `api2.cursor.sh` 的真实地址。
- 映射只在透明入口运行期间有效，不持久化用户凭据。
- 未命中映射的连接立即关闭并记录脱敏原因。

### 透明 TLS listener

- 为每个 fake-IP 的 `443` 接收原始 TCP 连接。
- 从 ClientHello 读取 SNI；SNI 必须与 fake-IP 映射一致，或属于明确允许的 Cursor 域名。
- 使用现有 CC2CX CA 为目标主机签发短期叶证书。
- TLS 终止后协商 HTTP/2；不支持的 ALPN 或协议返回明确错误并关闭连接。
- 不把原始 TLS 字节流直接送到 Provider；所有可改写请求必须进入现有 Cursor backend。

### HTTP/2 路由适配

- 复用现有 `CursorRelay`、路由白名单、Connect protobuf 和 Provider bridge。
- 保留原始 Host/SNI 作为路由元数据，不转发客户端 Authorization/Cookie 到本地 backend。
- `/agent.v1.AgentService/Run`、`RunSSE`、`BidiAppend` 进入本地 backend。
- 账户、遥测和未实现路径按显式策略处理：本地最小响应、拒绝或安全旁路；默认不转入自定义模型 Provider。
- 每个请求记录脱敏的入口类型、主机、路径、协议、状态和 Provider 标识。

## 生命周期与回滚

模式 T：

1. 检查 CA 完整性和当前用户信任状态。
2. 检查 fake-IP/443 监听能力；不满足则不改 Hosts。
3. 写入 Hosts 标记块并保存事务备份。
4. 启动透明 listener 和本地 backend。
5. **不**启动给 Cursor 用的显式 HTTP 代理，**不**写入 `http.proxy` / `--proxy-server`。
6. 执行无请求体 preflight；失败则停止 listener、恢复 Hosts、释放映射。
7. 停止时先停透明入口，再恢复 Hosts。

模式 P：

1. 检查 CA。
2. 启动 backend 与显式 MITM。
3. 写入可逆 settings：`http.proxy`、`http.proxySupport=on`、`cursor.general.disableHttp2=true`（与 cursor-byok 一致）。
4. **不**写系统 Hosts，**不**绑定 fake-IP `:443`。
5. 失败则停代理并恢复 settings。

## 安全边界

- 默认只绑定 loopback，不监听局域网地址。
- fake-IP 地址池、Hosts 标记和 backend 地址必须互相校验。
- 不记录 token、Cookie、用户提示词、文件路径中的敏感部分或 CA 私钥。
- 证书匹配使用完整 DER/指纹，不使用主题名作为信任依据。
- 未识别 SNI、Host、ALPN、路径或帧格式均采用 fail-closed 行为。

## 验收标准

### 单元与集成测试

- Hosts 标记块的幂等写入、外部修改冲突和精确恢复。
- fake-IP 分配、释放、冲突和未知地址拒绝。
- SNI 与 fake-IP 不匹配时拒绝连接。
- TLS/ALPN 协商失败和客户端断开时资源释放。
- 透明入口将合成 HTTP/2/Connect 请求送入现有 backend。
- backend/Provider 失败时入口返回可诊断错误，不回退到官方请求。

### 真实 Cursor 验收

模式必须写进记录，不可混报。

模式 T：

- 启动参数**没有** `--proxy-server`，settings **没有** `http.proxy`。
- NodeService 已建立连接到 loopback fake-IP `:443`（例如 `127.0.0.2:443`），而不是 `127.0.0.1:<mitm>`。
- backend 出现 `AgentService/Run` 或等价路径，且 Provider health 不是 `not_observed`。

模式 P：

- 启动可有 `--proxy-server` / `http.proxy`。
- **没有** CC2CX Hosts 块。
- NodeService 连接到本机 MITM 端口；backend 同样要有 Agent 路径和 Provider 证据。

两种模式都要求：日志不含凭证或提示词正文；停止后 Hosts/settings 按该模式回滚。Cursor `Auto` 官方回复不能当作接管证据。

## 实现顺序

1. 提取可测试的 Hosts 事务和 fake-IP 映射模块。
2. 增加透明 TLS listener 的最小握手测试。
3. 将解密后的 HTTP/2 流接入现有 Cursor 路由，不复制 Provider 逻辑。
4. 加入生命周期、回滚和故障注入测试。
5. 在隔离 Cursor profile 上按**单一入口模式**做 E2E：模式 T 不用代理启动；模式 P 不写 Hosts。禁止用「Hosts + --proxy-server」同时开着的窗口宣称透明入口已验收。
6. 只有真实入口证据稳定后，才评估默认 profile 的可选启用。
