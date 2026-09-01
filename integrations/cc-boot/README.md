# cc-boot

One-command bootstrap for AI coding CLI tools — installs, configures, and hands off to [CC Switch](https://github.com/nicepkg/cc-switch).

[English](#features) | [中文](#功能特性)

## Features

- **One-command setup** — Bootstrap 6 AI coding tools with a single command
- **Supported tools**: Claude Code, Codex CLI, Gemini CLI, OpenCode, OpenClaw, Claude Code Router (CCR)
- **MCP integration** — Auto-configure popular MCP services (Brave Search, Exa, Tavily, GitHub, GitLab, Slack, Notion, Linear, etc.)
- **API provider presets** — Quick configuration for various API providers (China & Global)
- **Silent mode** — Fully non-interactive for CI/scripts: `--silent -p <provider> -k <key>`
- **Doctor diagnostics** — Verify your environment setup with `cc-boot doctor`
- **Self-update** — Stay current with `cc-boot update`
- **CC Switch handoff** — Seamlessly export config to CC Switch for tool switching
- **Bilingual** — Full Chinese (zh-CN) and English UI support

## Install

```bash
# macOS / Linux (auto-installs Node.js LTS 22.x if missing)
curl -fsSL https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.sh | bash

# Pin a specific Node.js major version (e.g. 20)
bash <(curl -fsSL https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.sh) --node-version 20

# Windows (PowerShell) — auto-installs Node.js LTS if missing
irm https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.ps1 | iex

# macOS via Homebrew
brew install wh000wh000/cc-boot/cc-boot

# npx (if Node.js already installed)
npx @haibane/cc-boot

# Or install globally
npm i -g @haibane/cc-boot
```

## Usage

```bash
# Interactive menu (default)
cc-boot

# Full initialization
cc-boot init
cc-boot init --all                        # Install all 6 tools
cc-boot init --tools cc,gem,oc            # Install specific tools

# Single tool setup
cc-boot setup claude-code
cc-boot setup gemini-cli

# Environment diagnostics
cc-boot doctor

# Check for / install updates
cc-boot update
cc-boot update --check                    # Check only, don't upgrade

# Hand off to CC Switch
cc-boot handoff
cc-boot handoff --install
```

### Silent / CI Mode

Run fully non-interactively — great for dotfiles, onboarding scripts, or CI pipelines:

```bash
# Install all tools with a specific provider + API key
cc-boot init --all --silent \
  --provider 302ai \
  --api-key sk-your-key

# Install specific tools, override the default model
cc-boot init --tools cc,oc --silent \
  --provider deepseek \
  --api-key sk-ds-xxx \
  --model deepseek-reasoner

# Use a custom API URL
cc-boot init --tools cc --silent \
  --provider openai \
  --api-key sk-xxx \
  --api-url https://api.your-proxy.com

# Chinese UI, set AI output language to English
cc-boot init --all --silent \
  --provider siliconflow \
  --api-key sk-xxx \
  --lang zh-CN \
  --ai-lang en
```

### CLI Options

| Flag | Description |
|------|-------------|
| `--all` | Install all 6 tools |
| `--tools <list>` | Comma-separated tool IDs (see aliases below) |
| `-s, --silent` | Non-interactive mode |
| `-p, --provider <name>` | API provider preset id or name (e.g. `302ai`, `deepseek`, `openai`) |
| `-k, --api-key <key>` | API key |
| `-u, --api-url <url>` | Custom API URL (overrides provider default) |
| `-m, --model <model>` | Override default model for the selected provider |
| `--lang <lang>` | UI language (`zh-CN` \| `en`) |
| `--ai-lang <lang>` | AI output language |
| `--mcp <services>` | Comma-separated MCP services to enable |
| `--no-ccswitch` | Skip CC Switch installation |

### Tool IDs & Aliases

| Tool | Full ID | Aliases |
|------|---------|---------|
| Claude Code | `claude-code` | `cc`, `claude` |
| Codex CLI | `codex` | `cx`, `openai` |
| Gemini CLI | `gemini-cli` | `gem`, `gemini` |
| OpenCode | `opencode` | `oc` |
| OpenClaw | `openclaw` | `claw` |
| Claude Code Router | `ccr` | `router`, `claude-code-router` |

### API Provider Presets

**China Region**

| ID | Name | Description |
|----|------|-------------|
| `302ai` | 302.AI | Multi-model gateway (Claude/GPT/Gemini) |
| `siliconflow` | SiliconFlow | High-performance inference |
| `deepseek` | DeepSeek | DeepSeek V3/R1 models |
| `moonshot` | Moonshot AI (Kimi) | Long-context models |
| `zhipu` | Zhipu AI (GLM) | GLM series |
| `qwen` | Alibaba Qwen | Qwen series |

**Global**

| ID | Name | Description |
|----|------|-------------|
| `anthropic` | Anthropic | Official Claude API |
| `openai` | OpenAI | GPT & o-series |
| `gemini` | Google Gemini | Gemini models |
| `groq` | Groq | Ultra-fast inference |

> Use fuzzy matching: `--provider deepseek` or `--provider DeepSeek` both work.

## Programmatic API

```ts
import {
  TOOL_TYPES,
  TOOL_LABELS,
  TOOL_ALIASES,
  type ToolType,
} from '@haibane/cc-boot'

// TOOL_TYPES = ['claude-code', 'codex', 'gemini-cli', 'opencode', 'openclaw', 'ccr']
// TOOL_LABELS = { 'claude-code': 'Claude Code (Anthropic)', ... }
// TOOL_ALIASES = { cc: 'claude-code', cx: 'codex', gem: 'gemini-cli', ... }
```

## Requirements

- Node.js >= 18 (installer auto-upgrades to LTS 22.x if needed)

## License

MIT

---

# cc-boot

一键引导 AI 编程 CLI 工具 — 安装、配置，然后交接给 [CC Switch](https://github.com/nicepkg/cc-switch)。

## 功能特性

- **一键配置** — 一条命令引导 6 个 AI 编程工具
- **支持工具**: Claude Code, Codex CLI, Gemini CLI, OpenCode, OpenClaw, Claude Code Router (CCR)
- **MCP 集成** — 自动配置热门 MCP 服务（Brave Search, Exa, Tavily, GitHub, GitLab, Slack, Notion, Linear 等）
- **API 供应商预设** — 快速配置各种 API 供应商（国内 & 海外）
- **静默模式** — 完全非交互式，适合 CI/脚本：`--silent -p <provider> -k <key>`
- **Doctor 诊断** — 通过 `cc-boot doctor` 验证环境配置
- **自动更新** — `cc-boot update` 保持最新版本
- **CC Switch 交接** — 无缝导出配置到 CC Switch 实现工具切换
- **双语** — 完整的中文 (zh-CN) 和英文界面支持

## 安装

```bash
# macOS / Linux（自动安装 Node.js LTS 22.x）
curl -fsSL https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.sh | bash

# Windows (PowerShell，自动安装 Node.js LTS)
irm https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.ps1 | iex

# macOS 通过 Homebrew
brew install wh000wh000/cc-boot/cc-boot

# npx（已有 Node.js 的情况）
npx @haibane/cc-boot

# 或全局安装
npm i -g @haibane/cc-boot
```

## 使用方式

```bash
# 交互式菜单（默认）
cc-boot

# 完整初始化
cc-boot init
cc-boot init --all                        # 安装全部 6 个工具
cc-boot init --tools cc,gem,oc            # 安装指定工具

# 单个工具配置
cc-boot setup claude-code
cc-boot setup gemini-cli

# 环境诊断
cc-boot doctor

# 检查 / 安装更新
cc-boot update
cc-boot update --check                    # 仅检查，不升级

# 交接到 CC Switch
cc-boot handoff
cc-boot handoff --install
```

### 静默模式 / CI 模式

完全非交互式运行，适合 dotfiles、团队配置脚本、CI 流水线：

```bash
# 安装所有工具，指定 provider 和 API key
cc-boot init --all --silent \
  --provider 302ai \
  --api-key sk-your-key

# 安装特定工具，覆盖默认模型
cc-boot init --tools cc,oc --silent \
  --provider deepseek \
  --api-key sk-ds-xxx \
  --model deepseek-reasoner

# 使用自定义 API URL
cc-boot init --tools cc --silent \
  --provider openai \
  --api-key sk-xxx \
  --api-url https://api.your-proxy.com
```

### CLI 选项

| 选项 | 说明 |
|------|------|
| `--all` | 安装全部 6 个工具 |
| `--tools <列表>` | 逗号分隔的工具 ID（见下方别名表） |
| `-s, --silent` | 非交互模式 |
| `-p, --provider <名称>` | API 供应商预设 ID 或名称（如 `302ai`、`deepseek`、`openai`） |
| `-k, --api-key <key>` | API Key |
| `-u, --api-url <url>` | 自定义 API URL（覆盖供应商默认值） |
| `-m, --model <model>` | 覆盖供应商的默认模型 |
| `--lang <lang>` | 界面语言（`zh-CN` \| `en`） |
| `--ai-lang <lang>` | AI 输出语言 |
| `--mcp <服务>` | 逗号分隔的 MCP 服务列表 |
| `--no-ccswitch` | 跳过 CC Switch 安装 |

### 工具 ID 与别名

| 工具 | 完整 ID | 别名 |
|------|---------|------|
| Claude Code | `claude-code` | `cc`, `claude` |
| Codex CLI | `codex` | `cx`, `openai` |
| Gemini CLI | `gemini-cli` | `gem`, `gemini` |
| OpenCode | `opencode` | `oc` |
| OpenClaw | `openclaw` | `claw` |
| Claude Code Router | `ccr` | `router`, `claude-code-router` |

## 环境要求

- Node.js >= 18（安装脚本会自动升级到 LTS 22.x）

## 许可证

MIT
