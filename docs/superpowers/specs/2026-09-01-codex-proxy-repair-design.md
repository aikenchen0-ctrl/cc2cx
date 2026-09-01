# Codex 网络修复设计

## 目标

在现有全局出站代理设置中增加一个可选的 Codex 同步开关，用于修复 Codex CLI 和 app-server 的 HTTPS/WebSocket 连接。该功能不得修改系统环境变量，不承诺完全控制 Codex 桌面应用的 Chromium 网络栈。

## 数据流

1. 用户配置或扫描得到 HTTP、HTTPS、SOCKS5 代理地址。
2. 启用 Codex 同步前，复用现有代理测试访问 ChatGPT/OpenAI HTTPS 端点。
3. 后端在当前 Codex 配置目录的 `.env` 中维护带边界标记的 CC2CX 专属块。
4. Codex 完全重启后读取代理变量；状态接口检查本地端口是否仍在监听。

## 写入规则

- HTTP/HTTPS：写入 `HTTP_PROXY`、`HTTPS_PROXY`、`NO_PROXY`。
- SOCKS5/SOCKS5H：写入 `ALL_PROXY`、`NO_PROXY`。
- 不接受其他协议、换行或双引号，避免 dotenv 注入。
- 只替换 `BEGIN/END CC2CX CODEX PROXY` 之间的内容。
- 保留用户所有其他 `.env` 行。
- 写入前创建私密备份，最终使用同目录原子替换。
- 符号链接、非 UTF-8 文件和不完整边界标记均停止写入并返回错误。

## 状态与提示

- 显示实际 `.env` 路径和启用状态。
- 只返回脱敏后的代理地址。
- 本地端口没有监听时显示高优先级警告。
- 检测到 `.env.txt` 时提示 Windows 隐藏扩展名问题。
- 每次修改后提示完全退出并重新启动 Codex。

## 平台边界

- Codex CLI/app-server：当前 Codex 源码会加载 `$CODEX_HOME/.env`，优先支持。
- Codex 桌面应用：环境变量对其 app-server 可能有效，但 Chromium UI 可能仍需要平台专用启动参数，因此不做完全生效承诺。
- Codex `features.network_proxy`：只管理沙箱子进程网络，不替代本功能。

## 验收

- 纯函数测试覆盖代理协议、受控块替换、关闭清理和注入拒绝。
- UI 测试覆盖启用前连接测试以及开关写入。
- TypeScript、Rust 检查、格式检查和现有测试通过。
