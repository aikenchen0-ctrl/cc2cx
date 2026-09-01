import { homedir } from 'node:os'
import { join } from 'pathe'
import {
  CLAUDE_DIR,
  CLAUDE_MD_FILE,
  CODEX_DIR,
  CODEX_CONFIG_FILE,
  CODEX_AUTH_FILE,
  SETTINGS_FILE,
  ZCF_CONFIG_DIR,
} from 'zcf'

// Re-export zcf constants
export {
  CLAUDE_DIR,
  CLAUDE_MD_FILE,
  CODEX_DIR,
  CODEX_CONFIG_FILE,
  CODEX_AUTH_FILE,
  SETTINGS_FILE,
  ZCF_CONFIG_DIR,
}

// cc-boot own config
export const CC_BOOT_DIR = join(homedir(), '.cc-boot')
export const CC_BOOT_CONFIG_FILE = join(CC_BOOT_DIR, 'config.toml')

// Gemini CLI
export const GEMINI_CONFIG_DIR = join(homedir(), '.gemini')
export const GEMINI_SETTINGS_FILE = join(GEMINI_CONFIG_DIR, 'settings.json')

// OpenCode
export const OPENCODE_CONFIG_DIR = join(homedir(), '.opencode')
export const OPENCODE_CONFIG_FILE = join(OPENCODE_CONFIG_DIR, 'config.toml')

// OpenClaw
export const OPENCLAW_CONFIG_DIR = join(homedir(), '.openclaw')
export const OPENCLAW_CONFIG_FILE = join(OPENCLAW_CONFIG_DIR, 'config.json')

// cc-launch — upstream: farion1231/cc-switch (cross-platform)
export const CC_LAUNCH_APP_ID = 'com.cclaunch.desktop'
export const CC_LAUNCH_BREW_CASK = 'cc-launch'
export const CC_LAUNCH_DEEP_LINK_PREFIX = 'cclaunch://'
export const CC_LAUNCH_REPO = 'farion1231/cc-switch'
export const CC_LAUNCH_RELEASES_URL = `https://github.com/${CC_LAUNCH_REPO}/releases/latest`

// Supported tools
export const TOOL_TYPES = ['claude-code', 'codex', 'gemini-cli', 'opencode', 'openclaw', 'ccr'] as const
export type ToolType = (typeof TOOL_TYPES)[number]

export const TOOL_ALIASES: Record<string, ToolType> = {
  cc: 'claude-code',
  claude: 'claude-code',
  cx: 'codex',
  openai: 'codex',
  gem: 'gemini-cli',
  gemini: 'gemini-cli',
  oc: 'opencode',
  claw: 'openclaw',
  router: 'ccr',
  'claude-code-router': 'ccr',
}

export const TOOL_LABELS: Record<ToolType, string> = {
  'claude-code': 'Claude Code (Anthropic)',
  'codex': 'Codex CLI (OpenAI)',
  'gemini-cli': 'Gemini CLI (Google)',
  'opencode': 'OpenCode',
  'openclaw': 'OpenClaw',
  'ccr': 'Claude Code Router (CCR)',
}

// Workflow namespace
export const WORKFLOW_NAMESPACE = 'zcf'
export const CLAUDE_COMMANDS_DIR = join(CLAUDE_DIR, 'commands', WORKFLOW_NAMESPACE)

// Block markers for CLAUDE.md
export const BLOCK_START = '<!-- cc-boot:start -->'
export const BLOCK_END = '<!-- cc-boot:end -->'
