import {
  isClaudeCodeInstalled,
  installClaudeCode,
  detectInstalledVersion,
  configureApi,
  writeMcpConfig,
  mergeMcpServers,
  readMcpConfig,
  ensureClaudeDir,
} from 'zcf'
import type { ClaudeConfiguration, McpServerConfig } from 'zcf'

import { CLAUDE_DIR, SETTINGS_FILE } from '../constants.js'
import type { McpServerEntry, ProviderConfig, ToolAdapter, ToolConfigPaths } from './types.js'

export class ClaudeCodeAdapter implements ToolAdapter {
  readonly type = 'claude-code' as const
  readonly label = 'Claude Code (Anthropic)'
  readonly command = 'claude'

  async isInstalled(): Promise<boolean> {
    return isClaudeCodeInstalled()
  }

  async install(_silent?: boolean): Promise<void> {
    await installClaudeCode()
  }

  async detectVersion(): Promise<string | null> {
    return detectInstalledVersion('claude-code')
  }

  async configureProvider(config: ProviderConfig): Promise<void> {
    configureApi({
      key: config.apiKey,
      url: config.apiUrl,
      authType: config.authType === 'auth_token' ? 'auth_token' : 'api_key',
    })
  }

  async configureMcp(servers: Record<string, McpServerEntry>): Promise<void> {
    ensureClaudeDir()
    const existing = readMcpConfig()
    // Convert McpServerEntry to McpServerConfig for zcf compatibility
    const zcfServers: Record<string, McpServerConfig> = {}
    for (const [name, entry] of Object.entries(servers)) {
      zcfServers[name] = {
        command: entry.command ?? '',
        args: entry.args,
        env: entry.env,
      } as McpServerConfig
    }
    const merged = mergeMcpServers(existing, zcfServers)
    writeMcpConfig(merged)
  }

  getConfigPaths(): ToolConfigPaths {
    return {
      configDir: CLAUDE_DIR,
      settingsFile: SETTINGS_FILE,
      mcpFile: SETTINGS_FILE,
    }
  }
}
