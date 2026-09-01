import fs from 'fs-extra'
import { join } from 'pathe'
import {
  readMcpConfig,
  writeMcpConfig,
  mergeMcpServers,
} from 'zcf'
import type { McpServerConfig } from 'zcf'
import type { ToolType } from '../constants.js'
import {
  CODEX_DIR,
  CODEX_CONFIG_FILE,
  GEMINI_CONFIG_DIR,
  GEMINI_SETTINGS_FILE,
  OPENCODE_CONFIG_DIR,
  OPENCODE_CONFIG_FILE,
  OPENCLAW_CONFIG_DIR,
  OPENCLAW_CONFIG_FILE,
} from '../constants.js'
import type { McpServerEntry } from '../tools/types.js'
import type { McpServicePreset } from './presets.js'

/**
 * Build McpServerEntry records from selected services + collected API keys.
 */
export function buildMcpServers(
  services: McpServicePreset[],
  apiKeys: Record<string, string>,
): Record<string, McpServerEntry> {
  const servers: Record<string, McpServerEntry> = {}

  for (const service of services) {
    const entry: McpServerEntry = { ...service.config }

    // Inject API key into env if required
    if (service.requiresApiKey && service.apiKeyEnvVar && apiKeys[service.id]) {
      entry.env = {
        ...entry.env,
        [service.apiKeyEnvVar]: apiKeys[service.id],
      }
    }

    servers[service.id] = entry
  }

  return servers
}

/**
 * Write MCP configuration for the specified tool type.
 * Delegates to tool-specific writers.
 */
export async function writeMcpForTool(
  tool: ToolType,
  servers: Record<string, McpServerEntry>,
): Promise<void> {
  switch (tool) {
    case 'claude-code':
      return writeClaudeCodeMcp(servers)
    case 'codex':
      return writeCodexMcp(servers)
    case 'gemini-cli':
      return writeGeminiMcp(servers)
    case 'opencode':
      return writeOpenCodeMcp(servers)
    case 'openclaw':
      return writeOpenClawMcp(servers)
  }
}

/**
 * Write MCP config for all selected tools at once.
 */
export async function writeMcpForTools(
  tools: ToolType[],
  servers: Record<string, McpServerEntry>,
): Promise<void> {
  for (const tool of tools) {
    await writeMcpForTool(tool, servers)
  }
}

// =============================================================================
// Tool-specific MCP writers
// =============================================================================

/** Claude Code: uses zcf's readMcpConfig/mergeMcpServers/writeMcpConfig */
async function writeClaudeCodeMcp(servers: Record<string, McpServerEntry>): Promise<void> {
  const existing = readMcpConfig()
  // Convert McpServerEntry to McpServerConfig (compatible superset)
  const zcfServers: Record<string, McpServerConfig> = {}
  for (const [name, entry] of Object.entries(servers)) {
    zcfServers[name] = {
      type: entry.type,
      command: entry.command,
      args: entry.args,
      url: entry.url,
      env: entry.env,
    }
  }
  const merged = mergeMcpServers(existing, zcfServers)
  writeMcpConfig(merged)
}

/** Codex CLI: writes to ~/.codex/config.yaml mcp_servers section */
async function writeCodexMcp(servers: Record<string, McpServerEntry>): Promise<void> {
  await fs.ensureDir(CODEX_DIR)

  // Codex uses a YAML-like config; we write a simple format
  let content = ''
  if (await fs.pathExists(CODEX_CONFIG_FILE)) {
    content = await fs.readFile(CODEX_CONFIG_FILE, 'utf-8')
  }

  // Remove existing mcp_servers section if present
  const mcpStart = content.indexOf('mcp_servers:')
  if (mcpStart !== -1) {
    // Find next top-level key or end of file
    const rest = content.slice(mcpStart)
    const lines = rest.split('\n')
    let endIdx = lines.length
    for (let i = 1; i < lines.length; i++) {
      if (lines[i].length > 0 && !lines[i].startsWith(' ') && !lines[i].startsWith('\t')) {
        endIdx = i
        break
      }
    }
    const before = content.slice(0, mcpStart)
    const after = lines.slice(endIdx).join('\n')
    content = before + after
  }

  // Append mcp_servers section
  const mcpYaml = buildCodexMcpYaml(servers)
  content = content.trimEnd() + '\n\n' + mcpYaml + '\n'

  await fs.writeFile(CODEX_CONFIG_FILE, content, 'utf-8')
}

/** Gemini CLI: writes to ~/.gemini/settings.json */
async function writeGeminiMcp(servers: Record<string, McpServerEntry>): Promise<void> {
  await fs.ensureDir(GEMINI_CONFIG_DIR)

  let config: Record<string, any> = {}
  if (await fs.pathExists(GEMINI_SETTINGS_FILE)) {
    try {
      config = await fs.readJson(GEMINI_SETTINGS_FILE)
    }
    catch { /* start fresh */ }
  }

  // Gemini CLI uses mcpServers in settings.json
  config.mcpServers = {
    ...(config.mcpServers || {}),
    ...convertToGeminiFormat(servers),
  }

  await fs.writeJson(GEMINI_SETTINGS_FILE, config, { spaces: 2 })
}

/** OpenCode: writes to ~/.opencode/config.toml [mcp] section */
async function writeOpenCodeMcp(servers: Record<string, McpServerEntry>): Promise<void> {
  await fs.ensureDir(OPENCODE_CONFIG_DIR)

  let content = ''
  if (await fs.pathExists(OPENCODE_CONFIG_FILE)) {
    content = await fs.readFile(OPENCODE_CONFIG_FILE, 'utf-8')
  }

  // Remove existing [mcp.*] sections
  const lines = content.split('\n')
  const filtered: string[] = []
  let inMcp = false
  for (const line of lines) {
    if (line.match(/^\[mcp\./)) {
      inMcp = true
      continue
    }
    if (inMcp && line.startsWith('[')) {
      inMcp = false
    }
    if (!inMcp) {
      filtered.push(line)
    }
  }
  content = filtered.join('\n').trimEnd()

  // Append [mcp.*] sections
  const mcpToml = buildOpenCodeMcpToml(servers)
  content = content + '\n\n' + mcpToml + '\n'

  await fs.writeFile(OPENCODE_CONFIG_FILE, content, 'utf-8')
}

/** OpenClaw: writes to ~/.openclaw/config.json */
async function writeOpenClawMcp(servers: Record<string, McpServerEntry>): Promise<void> {
  await fs.ensureDir(OPENCLAW_CONFIG_DIR)

  let config: Record<string, any> = {}
  if (await fs.pathExists(OPENCLAW_CONFIG_FILE)) {
    try {
      config = await fs.readJson(OPENCLAW_CONFIG_FILE)
    }
    catch { /* start fresh */ }
  }

  config.mcpServers = {
    ...(config.mcpServers || {}),
    ...servers,
  }

  await fs.writeJson(OPENCLAW_CONFIG_FILE, config, { spaces: 2 })
}

// =============================================================================
// Format helpers
// =============================================================================

function buildCodexMcpYaml(servers: Record<string, McpServerEntry>): string {
  const lines: string[] = ['mcp_servers:']
  for (const [name, server] of Object.entries(servers)) {
    lines.push(`  - name: ${name}`)
    lines.push(`    type: ${server.type}`)
    if (server.command) {
      lines.push(`    command: ${server.command}`)
    }
    if (server.args && server.args.length > 0) {
      lines.push(`    args:`)
      for (const arg of server.args) {
        lines.push(`      - "${arg}"`)
      }
    }
    if (server.url) {
      lines.push(`    url: ${server.url}`)
    }
    if (server.env && Object.keys(server.env).length > 0) {
      lines.push(`    env:`)
      for (const [k, v] of Object.entries(server.env)) {
        lines.push(`      ${k}: "${v}"`)
      }
    }
  }
  return lines.join('\n')
}

function convertToGeminiFormat(servers: Record<string, McpServerEntry>): Record<string, any> {
  const result: Record<string, any> = {}
  for (const [name, server] of Object.entries(servers)) {
    result[name] = {
      command: server.command,
      args: server.args || [],
      ...(server.env && Object.keys(server.env).length > 0 ? { env: server.env } : {}),
    }
  }
  return result
}

function buildOpenCodeMcpToml(servers: Record<string, McpServerEntry>): string {
  const lines: string[] = []
  for (const [name, server] of Object.entries(servers)) {
    lines.push(`[mcp.${name}]`)
    lines.push(`type = "${server.type}"`)
    if (server.command) {
      lines.push(`command = "${server.command}"`)
    }
    if (server.args && server.args.length > 0) {
      lines.push(`args = [${server.args.map(a => `"${a}"`).join(', ')}]`)
    }
    if (server.url) {
      lines.push(`url = "${server.url}"`)
    }
    if (server.env && Object.keys(server.env).length > 0) {
      lines.push('')
      lines.push(`[mcp.${name}.env]`)
      for (const [k, v] of Object.entries(server.env)) {
        lines.push(`${k} = "${v}"`)
      }
    }
    lines.push('')
  }
  return lines.join('\n').trimEnd()
}
