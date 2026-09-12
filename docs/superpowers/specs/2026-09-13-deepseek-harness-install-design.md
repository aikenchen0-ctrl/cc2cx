# DeepSeek Harness 自动安装设计

## 目标

在现有“安装 Agent”面板中增加 DeepSeek Harness 作为独立受管 Agent，并通过官方 npm 包完成可重复安装和安装后验证。

## 安装对象

固定使用参考项目已验证的版本：

```text
@deepseek-ai/dsh@0.1.1-rc.2
@deepseek-ai/dsh-acp@0.1.1-rc.2
```

`dsh` 是用户可执行的 DeepSeek Harness CLI；`dsh-acp` 是其 ACP 插件，安装到 `cc2cx` profile 中。安装器不写入 API Key，不登录账号，也不修改其他 Agent 配置。

## 现有安装器接入

- 增加 `AgentInstallSpec { id: "deepseek", tool: "dsh" }`。
- `dsh` 纳入工具版本探测和搜索路径。
- 安装命令复用现有 Node.js 依赖检查、用户 npm prefix、官方 npm 源后接国内镜像回退。
- 在 CLI 安装成功后执行 `dsh plugin --profile cc2cx add "@deepseek-ai/dsh-acp@0.1.1-rc.2"`。
- 安装后重新执行 `dsh --version`；CLI 和 ACP 插件任一失败都报告安装失败。
- Windows、macOS、Linux 使用同一 npm 主路径；不新增 shell installer 或系统级提权。

## UI 与状态

DeepSeek Harness 使用现有 AgentInstallStatus、AgentInstallOutput 和 AgentInstallProgress，无新增前端协议。面板展示安装状态、版本、依赖和组合安装进度；安装完成后重新查询状态。

## 错误与回滚

- Node.js/npm 缺失时沿用现有 `ensure_node_runtime`。
- npm 主源失败时按既有顺序尝试 `registry.npmmirror.com` 和华为云 npm 镜像。
- CLI 已安装但 ACP 插件注册失败时返回失败，不显示成功；允许用户重试。
- 不删除用户已有的 `dsh`、`.dsh` 配置或凭据文件。

## 验收

- 安装列表包含 DeepSeek Harness，未安装时显示可执行命令。
- 命令包含两个固定版本包及 ACP profile 注册。
- 状态探测能识别 `dsh` 已安装、可运行和版本。
- 安装失败不会影响其他 Agent 安装。
- 安装命令和日志不包含 API Key、Cookie 或完整用户提示词。

## 实现状态

- Rust AgentInstallSpec、工具探测、Node.js 依赖、npm 镜像回退和固定版本安装命令已接入。
- CLI 安装后会注册 `cc2cx` ACP profile；CLI 或 ACP profile 任一不可用时，状态不会显示为已安装。
- 前端已增加 DeepSeek Harness 使用提示和启动入口，复用现有安装确认与进度流程。
- 已通过 5 项 DeepSeek Rust 单元测试、`rustfmt --check`（`misc.rs`）和 Debug 构建；真实机器安装尚未执行。
- 工作区级 `cargo fmt --all --check` 仍受既有 Cursor 文件格式差异影响，全量 Cargo 集成测试则被既有 `cursor_e2e_contract` 未解析导入阻断；两者均未归因于本功能。
