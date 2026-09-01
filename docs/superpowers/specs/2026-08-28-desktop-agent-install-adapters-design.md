# 桌面 Agent 安装适配器设计

## 目标

在现有 Agent 首次启动检查和批量安装流程中，补齐 Codex UI 与 WorkBuddy 的桌面应用安装、检测和启动能力，同时保留 Claude Code、Codex CLI、OpenCode 的 Node.js 依赖自动准备。

## 适配边界

- CLI Agent 继续使用现有生命周期命令。
- Codex UI 在 Windows 使用官方 Microsoft Store 包，在 macOS 使用 Homebrew 官方 cask 元数据提供的 ChatGPT 桌面包；按 Intel/Apple Silicon 选择变体并校验后安装。检测 Codex/ChatGPT 应用目录及 Windows 应用包。
- WorkBuddy 使用官网更新接口按平台和架构获取安装包，下载后校验 SHA-256，再执行 Windows 安装器或 macOS 应用安装。
- 不使用第三方镜像，不绕过系统签名和 Gatekeeper，不静默提升权限。

## 数据流

1. 后端返回统一的 AgentInstallStatus。
2. 桌面适配器根据当前平台返回状态和安装命令描述。
3. 前端批量安装按状态执行，Node.js 依赖只对 CLI Agent 触发。
4. 桌面安装成功后重新检测并启动已安装应用。

## 错误处理

- 官方接口不可用、架构不支持、下载失败、哈希不匹配或安装器退出失败时，返回可读错误并保留重试入口。
- WorkBuddy 的安装包必须在写入应用目录前完成哈希校验。
- 桌面应用检测不到版本时仍可报告已安装和可启动，版本字段允许为空。

## 验证

- 单元测试覆盖平台/架构路由、安装状态检测、WorkBuddy 哈希校验和失败分支。
- 前端测试覆盖五个目标 Agent 在首启面板中的展示和批量执行顺序。
- TypeScript、Rust 格式检查和可用的编译检查在提交前执行。
