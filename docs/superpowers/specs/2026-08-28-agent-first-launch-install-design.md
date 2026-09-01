# Agent 首次启动安装引导设计

## 目标

在 Windows 和 macOS 上，应用首次启动后自动检查 Codex UI、Codex CLI、Claude Code、OpenCode 和 WorkBuddy 的安装状态；发现缺失项时打开安装引导。对于依赖 Node.js 的 Codex CLI、Claude Code 和 OpenCode，在安装 Agent 前自动补齐 Node.js，并在安装完成后重新检测。

## 当前状态

- 前端已有 `AgentInstallPanel`，支持状态展示、单个 Agent 安装、实时输出和安装后刷新。
- Rust 后端已有 `get_agent_install_statuses` 与 `run_agent_install` 命令，并能生成平台相关的 Agent 安装命令。
- 首启逻辑目前不会主动打开 Agent 安装面板。
- Node.js 只作为依赖状态显示，缺失时没有由桌面安装流程统一补齐。
- `integrations/cc-boot` 保留为独立 CLI 集成，不作为桌面端首启流程的运行时依赖。

## 术语与范围

### 目标 Agent

| 标识 | 类型 | Node.js 依赖 | 首版安装方式 |
| --- | --- | --- | --- |
| `codex-ui` | 桌面应用 | 否 | 官方桌面发行包或现有桌面适配器 |
| `codex` | 命令行工具 | 是 | 官方命令或 npm 安装命令 |
| `claude` | 命令行工具 | 是 | 官方命令或 npm 安装命令 |
| `opencode` | 命令行工具 | 是 | 官方命令或 npm 安装命令 |
| `workbuddy` | 桌面应用 | 否 | 官方桌面发行包或现有桌面适配器 |

本次不包含 API Key 写入、供应商配置、代理路由、账号登录、会话迁移和第三方镜像配置。

## 首启状态机

```text
应用启动
  -> 读取本地首启检查标记
  -> 未检查：查询 Agent 状态
       -> 存在缺失或不可运行项：打开安装面板
       -> 全部正常：写入已检查标记
  -> 已检查：不自动弹出，仅保留手动重新检查
```

首启检查标记只表示“完成过一次状态扫描”，不表示所有 Agent 都已经安装。用户可在安装面板中手动刷新并再次执行安装。

## Node.js 依赖编排

新增后端命令 `ensure_node_runtime`，返回结构化结果：

```text
{
  available: boolean,
  version: string | null,
  installed: boolean,
  command: string | null,
  error: string | null
}
```

规则：

1. 通过当前应用可见的 PATH 探测 `node --version` 和 `npm --version`。
2. Node.js 主版本大于等于 18 时直接复用，不重复安装。
3. Windows 优先使用 `winget` 安装 `OpenJS.NodeJS.LTS`；失败时尝试现有可用的 Chocolatey 或 Scoop；没有可用包管理器时返回人工安装提示。
4. macOS 优先使用 Homebrew 安装 Node.js LTS；没有 Homebrew 时使用 Node.js 官方安装包路径或返回人工安装提示。
5. 安装后重新探测 Node.js 与 npm，并将验证结果返回前端。
6. 命令执行必须复用现有 Agent 安装输出事件，避免引入第二套日志通道。
7. Node.js 安装失败只阻止三个 Node.js Agent，Codex UI 和 WorkBuddy 仍可继续安装。

## 前端交互

安装面板增加以下状态和动作：

- 顶部显示 Node.js 状态：已满足、需要安装、安装中、安装失败。
- Node.js 缺失且存在依赖 Agent 时，显示“安装 Node.js 并继续”动作。
- 增加“一键安装缺失项”动作，顺序为 Node.js、Codex CLI、Claude Code、OpenCode，再处理桌面 Agent。
- 现有单个安装按钮继续保留；当目标 Agent 依赖 Node.js 且 Node.js 缺失时，单个安装动作先执行 Node.js 检查与补齐。
- 单个 Agent 失败不终止批量安装；最终按 Agent 返回成功、跳过、失败三种结果。
- 安装完成后自动刷新状态，避免前端依赖旧状态推断结果。

## 后端接口边界

- 保留现有 `get_agent_install_statuses` 和 `run_agent_install` 命令的兼容性。
- 新增 `ensure_node_runtime`；前端只通过该命令获取 Node.js 结果，不直接拼接平台命令。
- `run_agent_install` 在执行 Node.js Agent 前调用依赖检查；如果 Node.js 不可用，返回明确错误并不启动必然失败的 npm 命令。
- Agent 安装任务仍使用现有输出事件 `agent-install-output`，事件中的 `agent_id` 使用 `node-runtime` 表示 Node.js 阶段。

## 权限与安全

- 默认使用当前用户范围安装，避免无提示提权。
- Windows 包管理器命令必须带接受协议参数，但不得绕过系统安全策略。
- macOS 不自动执行 `sudo`；需要管理员权限时向用户显示官方安装提示。
- 不记录 API Key、访问令牌或完整环境变量。
- 安装命令、下载源和验证结果写入现有日志，不保存敏感命令输出之外的额外数据。

## 测试验收

### 前端

- 未设置首启标记时，首次状态扫描会打开安装面板。
- 已设置首启标记时，启动不会重复打开安装面板。
- Node.js 缺失时，面板显示依赖安装动作。
- 一键安装会先等待 Node.js 成功，再安装依赖 Agent。
- 一个 Agent 失败时，其他独立 Agent 仍会继续。
- 安装结束后会重新调用状态查询。

### Rust

- Node.js 已满足版本时不会生成安装命令。
- Node.js 缺失时，Windows 和 macOS 返回各自平台的安装计划。
- Node.js 安装失败会返回结构化错误。
- 非 Node.js Agent 不会因为 Node.js 失败而被跳过。
- `agent-install-output` 能区分 Agent 安装阶段和 Node.js 阶段。

## 完成标准

1. 首次启动可以自动发现缺失 Agent 并打开安装引导。
2. 缺少 Node.js 时，用户无需手动打开终端即可在应用内完成 Node.js 补齐或获得可执行的官方安装提示。
3. Node.js 安装成功后，Codex CLI、Claude Code 和 OpenCode 能按顺序继续安装。
4. 任意单项失败不会隐藏其他结果，刷新后状态与系统实际安装状态一致。
5. 现有供应商、路由、会话和配置管理功能不受影响。
