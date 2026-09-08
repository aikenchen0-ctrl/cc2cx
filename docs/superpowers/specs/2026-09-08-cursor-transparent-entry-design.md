# Cursor 透明入口接管设计

状态：设计已批准，待实现计划

## 目标

让真实 Cursor 在不修改 Cursor 安装文件、账号数据库、会员状态或 storage 的前提下，将绕过 `http.proxy` 的 AI 网络连接导入 cc2cx：

```text
Cursor NodeService
  -> 受控域名解析/Hosts
  -> loopback fake-IP:443
  -> 透明 TLS 终止
  -> HTTP/2/Connect 请求
  -> 现有 Cursor backend
  -> 当前数据库 Provider endpoint/key
```

## 非目标

- 不伪造 Cursor 账号、订阅、会员或套餐权限。
- 不注入或修改 Cursor 安装目录中的 JavaScript、workbench、bootstrap 或 extension host。
- 不修改 Cursor 本地账号数据库或 storage。
- 不接管非 Cursor 域名。
- 不把未知协议降级为任意 Provider 请求。

## 组件

### Hosts 管理器

- 仅维护 `CC2CX` 标记块。
- 每次启用前保存原始文件哈希和原文。
- 只写入明确允许的 Cursor 主机名。
- 停用时仅在文件仍匹配“原文 + CC2CX 块”的预期状态时恢复；发生外部修改则拒绝覆盖并报告冲突。
- 所有启停操作可重复执行，失败时恢复已完成的部分。

### Fake-IP 映射器

- 从受控地址池分配 loopback 地址，例如 `127.0.0.2` 起的连续范围。
- 保存 `fake_ip -> hostname` 和 `hostname -> upstream candidates` 映射。
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

1. 检查 CA 完整性和当前用户信任状态。
2. 检查 fake-IP/443 监听能力；不满足则不改 Hosts。
3. 写入 Hosts 标记块并保存事务备份。
4. 启动透明 listener 和本地 backend。
5. 执行无请求体 preflight；失败则停止 listener、恢复 Hosts、释放映射。
6. 运行期间持续暴露入口、backend、Provider 健康状态。
7. 停止时先阻止新连接，再等待流式请求退出，最后恢复 Hosts 和 settings。
8. 崩溃恢复只处理 CC2CX 自己留下的事务备份，不覆盖用户后续修改。

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

- 启动参数和 profile 可唯一识别。
- NodeService 不再直接连接外部 Cursor AI 443，而是连接 loopback fake-IP/443。
- Cursor `Auto` 或合法自定义模型请求在 cc2cx backend 日志中出现。
- backend 的 Provider 证据显示请求命中当前配置 endpoint；日志不泄露认证值。
- 停止接入后默认 Cursor settings、Hosts 和证书状态可验证恢复。

## 实现顺序

1. 提取可测试的 Hosts 事务和 fake-IP 映射模块。
2. 增加透明 TLS listener 的最小握手测试。
3. 将解密后的 HTTP/2 流接入现有 Cursor 路由，不复制 Provider 逻辑。
4. 加入生命周期、回滚和故障注入测试。
5. 在隔离 Cursor profile 上进行真实 E2E。
6. 只有真实入口证据稳定后，才评估默认 profile 的可选启用。
