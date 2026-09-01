# cc-launch

`cc-launch` 是一个跨平台桌面应用，用于集中管理常用 AI 编程 Agent 的供应商配置、MCP、Skills、提示词和会话数据。它基于 Tauri 2 构建，支持 Windows、macOS 和 Linux。

> 打包后的应用不依赖 `http://localhost:3000/`。该地址只在 `pnpm dev` 开发模式下由 Vite 提供前端热更新服务。

## 支持的工具

| 工具 | 类型 |
| --- | --- |
| Claude Code | 命令行 Agent |
| Claude Desktop | 桌面客户端 |
| Codex | 命令行 Agent |
| Gemini CLI | 命令行 Agent |
| Grok Build | 命令行 Agent |
| OpenCode | 命令行 Agent |
| OpenClaw | Agent |
| Hermes | Agent |
| Pi | 命令行 Agent |

## 核心能力

- 管理多个模型供应商、API 地址、模型配置，并在受支持的客户端之间应用或切换配置。
- 扫描、安装和升级 Agent，保留 `cc-boot` 的 Agent 安装能力。
- 管理 MCP 服务器，支持导入、导出和向多个客户端应用配置。
- 管理 Skills、Prompt 与模板，便于在不同 Agent 间复用。
- 提供本地代理、故障转移、会话浏览、用量和价格配置等辅助能力。
- 支持配置目录覆写（包括 WSL 场景），以及 SQLite 数据的备份、恢复、导入和导出。
- 提供系统日志、托盘驻留和应用更新能力。

## 安装

从 [GitHub Releases](https://github.com/kodesh-talent/cc-launch/releases) 下载与操作系统匹配的安装包。

| 系统 | 推荐产物 | 安装方式 |
| --- | --- | --- |
| Windows | `.msi` | 双击运行安装向导，可选择安装目录。 |
| macOS | `.dmg` | 打开镜像后将 `cc-launch.app` 拖入 `Applications`。 |
| Linux | `.AppImage`、`.deb` 或 `.rpm` | AppImage 可直接运行；Deb/RPM 由发行版的软件安装工具安装。 |

不同系统的安装机制不同：Windows MSI 支持在向导中选择目录；macOS DMG 和 Linux Deb/RPM 使用系统约定的位置。完整的构建、签名和产物路径说明见 [产物构建.md](产物构建.md)。

## 快速开始

1. 在“安装 Agent”页面扫描本机已安装的工具，或安装所需 Agent。
2. 在“供应商”页面创建或导入供应商配置。
3. 选择目标客户端并应用配置。
4. 需要时重启对应的命令行工具或桌面客户端，使其读取新配置。
5. 在 MCP、Skills 和 Prompt 页面继续配置可复用能力。

## 数据与安全

默认应用数据目录为：

```text
~/.cc-launch/
```

主要数据库文件为：

```text
~/.cc-launch/cc-launch.db
```

可以在应用设置中修改配置目录。`cc-launch` 不会自动迁移旧版 `cc-switch` 的数据，二者保持隔离；如需迁移，请先备份原有数据，再通过应用的导入功能进行人工确认。

## 开发

### 环境要求

- Node.js 20 或更高版本
- Corepack 与 pnpm 10.12.3
- Rust 1.85 或更高版本
- 与当前操作系统对应的 [Tauri 前置依赖](https://v2.tauri.app/start/prerequisites/)

### 本地运行

在项目根目录执行：

```bash
corepack enable
pnpm install --frozen-lockfile
pnpm dev
```

`pnpm dev` 会启动 Vite 开发服务和 Tauri 桌面窗口。前端开发地址为 `http://localhost:3000/`，仅用于开发，不会被带入安装包。

### 常用检查

```bash
# TypeScript 类型检查
pnpm typecheck

# 前端单元测试
pnpm test:unit

# 前端格式检查
pnpm format:check

# Rust 格式与测试
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

## 构建

请在目标操作系统上构建相应平台的原生安装包，不能可靠地在一种桌面操作系统上交叉构建另一种系统的完整安装包。

Windows 本地构建可直接双击运行根目录的 `build-pack.cmd`，它会生成未签名的 MSI 供本地测试或内部使用。也可在 PowerShell 中执行：

```powershell
pnpm tauri build --bundles msi --config src-tauri/tauri.unsigned.conf.json
```

三端构建命令、系统依赖、签名要求和产物目录请阅读 [产物构建.md](产物构建.md)。

## 项目结构

```text
src/                    React + TypeScript 前端
src-tauri/              Tauri + Rust 后端、原生打包配置与测试
integrations/cc-boot/   Agent 安装与命令行集成
flatpak/                Linux Flatpak 打包相关文件
assets/                 图标与静态资源
```

## 贡献与安全

- 贡献说明：[CONTRIBUTING.md](CONTRIBUTING.md)
- 安全问题报告：[SECURITY.md](SECURITY.md)
- 使用支持：[SUPPORT.md](SUPPORT.md)

## 许可证

[MIT](LICENSE)
