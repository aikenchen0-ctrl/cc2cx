# 参考项目盘点

## 本次产物构建

- `src-tauri/tauri.conf.json` 已启用全部 bundle，并配置 Windows WiX 模板 `src-tauri/wix/per-user-main.wxs`。
- WiX 模板使用 `WixUI_InstallDir` 与 `INSTALLDIR`，MSI 安装向导支持选择安装目录。
- macOS 应使用原生 runner 构建通用 `universal-apple-darwin` 产物；Linux 应在 Linux runner 上构建 AppImage、Deb、RPM。
- 当前工作机为 Windows，因此本次只能原生验证 Windows MSI；三端命令已记录在 `产物构建.md`。

## cc-boot

目录分为 `commands`、`workflows`、`tools`、`providers`、`mcp`、`prompts`、`handoff`、`proxy`、`i18n` 和 `utils`。README 明确支持 Claude Code、Codex CLI、Gemini CLI、OpenCode、OpenClaw、CCR；提供 init/setup、静默模式、Provider 预设、自定义 URL/模型、MCP、doctor、update、handoff。

## cc-switch

前端模块包括 assets、components、contexts、hooks、config、commands、proxy、i18n、icons、lib、types、utils；后端包括 commands、database、deeplink、mcp、pi_config、proxy、resources、services、session_manager。README 明确覆盖 Provider 管理、代理与故障转移、MCP、Prompts、Skills、用量成本、会话/工作区、Deep Link、云同步、托盘、主题、开机启动、自动更新、备份和多语言。

## 约束

- cc-switch 当前是 Tauri 2 + Rust，Electron 迁移不能假设可直接复用 Rust IPC。
- 配置文件、SQLite 数据库、密钥存储和 CLI 行为必须兼容现有用户数据。
- 真实安装、写配置、代理和导入失败必须显式报错并可诊断，不能静默 fallback。
