import type { ToolType } from '../constants.js'

/**
 * Unified tool adapter interface — Strategy pattern
 * Each AI coding CLI tool implements this interface
 */
export interface ToolAdapter {
  /** Tool identifier */
  readonly type: ToolType

  /** Human-readable display name */
  readonly label: string

  /** CLI command name */
  readonly command: string

  /** Check if the tool is installed */
  isInstalled(): Promise<boolean>

  /** Install the tool (interactive or silent) */
  install(silent?: boolean): Promise<void>

  /** Detect installed version, null if not installed */
  detectVersion(): Promise<string | null>

  /** Write API/provider configuration */
  configureProvider(config: ProviderConfig): Promise<void>

  /** Write MCP server configuration (if supported) */
  configureMcp?(servers: Record<string, McpServerEntry>): Promise<void>

  /** Get the config file paths this tool uses */
  getConfigPaths(): ToolConfigPaths
}

export interface ProviderConfig {
  name: string
  apiKey: string
  apiUrl: string
  /** 'api_key' | 'auth_token' */
  authType: string
  model?: string
  haikuModel?: string
  sonnetModel?: string
  opusModel?: string
}

export interface McpServerEntry {
  type: 'stdio' | 'sse'
  command?: string
  args?: string[]
  url?: string
  env?: Record<string, string>
}

export interface ToolConfigPaths {
  configDir: string
  settingsFile: string
  mcpFile?: string
}
