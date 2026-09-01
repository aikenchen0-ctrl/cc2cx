# Agent 环境预检设计

## 目标

在现有 Agent 安装状态接口中加入安装前环境预检，让用户在点击安装前知道缺少哪些系统条件。预检必须区分可以由 CC Launch 自动修复的依赖、需要用户手动处理的依赖，以及只提供建议但不阻塞安装的依赖。

## 范围

本轮覆盖四类问题：

1. Windows 上 Claude Code 所需的 Git for Windows/Git Bash 或 WSL。
2. npm 全局安装目录的可写性和 PATH 可见性。
3. OpenCode Desktop 可能需要的 Microsoft Edge WebView2 Runtime。
4. WorkBuddy 启动故障常见的 .NET Runtime。

本轮不静默安装 Git、WSL、WebView2 或 .NET Runtime。它们可能触发管理员授权、系统重启、企业策略拦截或较大体积下载；预检应给出官方处理入口。Node.js 仍由既有自动安装链负责。

## Agent 与依赖映射

| Agent | 依赖 | 类型 | 是否阻塞 |
| --- | --- | --- | --- |
| Claude Code | Node.js 18+、npm | 运行时 | 是 |
| Claude Code（Windows） | Git Bash 或 WSL | Shell | 是 |
| Codex CLI | Node.js/npm，或官方独立二进制 | 运行时 | 是 |
| OpenCode CLI | Node.js/npm | 运行时 | 是 |
| OpenCode CLI（Windows） | WSL | 建议项 | 否 |
| Codex UI | 平台桌面安装能力 | 系统 | 是 |
| WorkBuddy | 平台桌面安装能力 | 系统 | 是 |
| WorkBuddy（Windows） | .NET Runtime | 启动建议 | 否 |
| OpenCode Desktop（未来支持） | WebView2 | 启动建议 | 否 |

OpenCode 当前由 CC Launch 安装命令行版本，因此 WebView2 只作为桌面版本的可扩展预检，不影响 CLI 安装。

## 数据结构

在现有 `AgentInstallDependency` 基础上增加可选字段，保持旧字段兼容：

```text
{
  name: string,
  available: boolean,
  detail: string,
  kind: "runtime" | "shell" | "system" | "permission" | "advisory",
  blocking: boolean,
  auto_fixable: boolean,
  action: string | null
}
```

字段语义：

- `available`：当前进程实际能够使用该依赖。
- `blocking`：为 true 时，安装按钮必须阻止并显示原因。
- `auto_fixable`：为 true 时，界面可以提供 CC Launch 内的自动修复动作；本轮只有 Node.js 和 npm 用户目录路径属于这一类。
- `action`：官方安装页或本地动作的中文说明，不包含命令行敏感信息。

## 检测规则

### Git Bash 与 WSL

- Windows 优先检查 `git.exe` 与常见 Git Bash 路径。
- 若未发现 Git Bash，再检查 `wsl.exe --status` 和至少一个可用发行版。
- 两者都不存在时，Claude Code 依赖标记为 `blocking: true`，动作提示分别指向 Git for Windows 和 WSL 官方安装说明。
- 非 Windows 平台不生成该依赖。

### npm 权限与 PATH

- 检查当前运行时可执行的 `node` 和 `npm`，同时检查 CC Launch 自带 Node 目录。
- 获取 npm global prefix，验证目录存在或可创建，并使用临时文件执行一次可写性测试；测试文件必须立即删除。
- 不修改用户现有全局 registry，不使用 `sudo npm install -g`。
- 全局目录不可写时，安装命令切换到用户目录方案；如果无法安全切换，依赖标记为阻塞并提供手动修复说明。
- GUI 启动时使用合并后的 PATH，避免安装成功但前端重启前找不到命令。

### WebView2

- 仅 Windows 检测。
- 检查 WebView2 Evergreen Runtime 的注册表项或可执行运行时目录。
- 当前 CLI Agent 不因 WebView2 缺失而阻塞；当未来安装 OpenCode Desktop 时再把该检查提升为阻塞条件。
- 检测结果只显示官方 Microsoft WebView2 安装入口，不执行不受控的第三方安装器。

### .NET Runtime

- 仅 Windows 检测 `dotnet --list-runtimes` 和常见安装目录。
- 缺失时作为 WorkBuddy 启动建议显示，不阻止下载安装包。
- WorkBuddy 安装完成后如果启动探测失败，错误信息必须包含 .NET Runtime 建议，而不是只显示“启动失败”。

## 前端交互

- Agent 卡片显示依赖状态：可用、可自动修复、需要手动处理、建议安装。
- 只有 `blocking: true && available: false` 时禁用对应 Agent 的安装按钮。
- 批量安装开始前重新读取状态；存在阻塞项时跳过该 Agent，继续处理其他独立 Agent，并在结果区列出跳过原因。
- 提供“重新检测”按钮。用户从官方页面完成安装后，不需要重启 CC Launch 即可刷新依赖状态。
- 进度面板继续复用 `agent-install-progress` 事件；预检失败使用 `phase: prepare`，不伪造下载进度。

## 后端接口边界

- 保留 `get_agent_install_statuses` 和 `run_agent_install` 的命令名称及现有字段。
- 依赖检测放在 Rust 后端，前端不执行 `where`、`which`、PowerShell 或注册表查询。
- `run_agent_install` 在执行前再次检查阻塞依赖，避免状态过期导致必然失败的安装命令被启动。
- 所有官方安装入口使用固定白名单，不能由前端传入任意 URL 或命令。

## 错误与安全

- 检测命令失败不能被解释为依赖已安装；必须返回 `available: false` 和可读原因。
- 临时权限测试文件使用随机文件名并在成功、失败和异常路径清理。
- 不记录完整环境变量、访问令牌、API Key 或 npm 配置中的敏感字段。
- 不绕过 Windows SmartScreen、PowerShell 执行策略、macOS Gatekeeper 或企业代理策略。

## 测试验收

### Rust

- Git Bash 和 WSL 的存在性映射到 Claude Code 的阻塞依赖。
- OpenCode 的 WSL 建议项不阻止 CLI 安装。
- npm global prefix 可写、不可写和不存在三种情况分别得到稳定状态。
- WebView2 与 .NET 缺失只产生建议，不阻塞 WorkBuddy/CLI 安装。
- `run_agent_install` 在阻塞依赖缺失时不启动子进程。

### 前端

- 依赖状态显示四种语义状态，长错误文本不溢出卡片。
- 阻塞依赖禁用安装按钮，建议依赖不禁用。
- 点击重新检测会刷新所有 Agent 状态。
- 批量安装跳过阻塞项后仍继续其他 Agent，并展示跳过原因。

## 完成标准

1. 用户在安装前能看到 Git/WSL、npm 权限、WebView2 和 .NET 的明确状态。
2. 必须依赖缺失时不会启动必然失败的安装进程。
3. 可自动修复项继续在应用内完成，手动依赖提供官方处理入口。
4. 不修改用户全局 npm registry，不静默提权，不执行来源不明的安装器。
5. 现有首启引导、进度事件、镜像回退和桌面安装流程保持兼容。
