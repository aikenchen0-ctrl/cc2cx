import type { McpServerEntry } from '../tools/types.js'

/** MCP service preset definition */
export interface McpServicePreset {
  /** Unique identifier */
  id: string
  /** Display name */
  name: string
  /** Short description */
  description: string
  /** Whether an API key is required */
  requiresApiKey: boolean
  /** Prompt shown when asking for API key */
  apiKeyPrompt?: string
  /** Placeholder for API key input */
  apiKeyPlaceholder?: string
  /** Environment variable name for the API key */
  apiKeyEnvVar?: string
  /** MCP server configuration */
  config: McpServerEntry
  /** Category for grouping */
  category: 'filesystem' | 'memory' | 'search' | 'browser' | 'developer' | 'productivity'
}

// =============================================================================
// MCP Service Presets
// =============================================================================

export const MCP_PRESETS: McpServicePreset[] = [
  // -- Filesystem --
  {
    id: 'filesystem',
    name: 'Filesystem',
    description: 'Read/write/search local files with configurable access',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-filesystem', '.'],
    },
    category: 'filesystem',
  },

  // -- Memory --
  {
    id: 'memory',
    name: 'Memory',
    description: 'Persistent knowledge graph for long-term memory',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-memory'],
    },
    category: 'memory',
  },

  // -- Search --
  {
    id: 'brave-search',
    name: 'Brave Search',
    description: 'Web and local search via Brave Search API',
    requiresApiKey: true,
    apiKeyPrompt: 'Enter Brave Search API key',
    apiKeyPlaceholder: 'BSA...',
    apiKeyEnvVar: 'BRAVE_API_KEY',
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-brave-search'],
      env: { BRAVE_API_KEY: '' },
    },
    category: 'search',
  },
  {
    id: 'exa',
    name: 'Exa Search',
    description: 'AI-native web search and content retrieval',
    requiresApiKey: true,
    apiKeyPrompt: 'Enter Exa API key',
    apiKeyPlaceholder: 'exa-...',
    apiKeyEnvVar: 'EXA_API_KEY',
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', 'exa-mcp-server'],
      env: { EXA_API_KEY: '' },
    },
    category: 'search',
  },
  {
    id: 'tavily',
    name: 'Tavily Search',
    description: 'AI-optimized search engine',
    requiresApiKey: true,
    apiKeyPrompt: 'Enter Tavily API key',
    apiKeyEnvVar: 'TAVILY_API_KEY',
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', 'tavily-mcp-server'],
      env: { TAVILY_API_KEY: '' },
    },
    category: 'search',
  },

  // -- Browser --
  {
    id: 'puppeteer',
    name: 'Puppeteer',
    description: 'Browser automation with screenshots and console access',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-puppeteer'],
    },
    category: 'browser',
  },
  {
    id: 'playwright',
    name: 'Playwright',
    description: 'Browser automation via Playwright',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@playwright/mcp@latest'],
    },
    category: 'browser',
  },
  {
    id: 'fetch',
    name: 'Fetch',
    description: 'Fetch web content and convert to markdown',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-fetch'],
    },
    category: 'browser',
  },

  // -- Developer --
  {
    id: 'github',
    name: 'GitHub',
    description: 'GitHub API integration (repos, issues, PRs)',
    requiresApiKey: true,
    apiKeyPrompt: 'Enter GitHub Personal Access Token',
    apiKeyPlaceholder: 'ghp_...',
    apiKeyEnvVar: 'GITHUB_PERSONAL_ACCESS_TOKEN',
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-github'],
      env: { GITHUB_PERSONAL_ACCESS_TOKEN: '' },
    },
    category: 'developer',
  },
  {
    id: 'gitlab',
    name: 'GitLab',
    description: 'GitLab API integration',
    requiresApiKey: true,
    apiKeyPrompt: 'Enter GitLab Personal Access Token',
    apiKeyEnvVar: 'GITLAB_PERSONAL_ACCESS_TOKEN',
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-gitlab'],
      env: { GITLAB_PERSONAL_ACCESS_TOKEN: '' },
    },
    category: 'developer',
  },
  {
    id: 'sequential-thinking',
    name: 'Sequential Thinking',
    description: 'Step-by-step reasoning and problem decomposition',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-sequential-thinking'],
    },
    category: 'developer',
  },
  {
    id: 'sqlite',
    name: 'SQLite',
    description: 'Read/write SQLite databases',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-sqlite'],
    },
    category: 'developer',
  },
  {
    id: 'postgres',
    name: 'PostgreSQL',
    description: 'Query PostgreSQL databases',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-postgres'],
    },
    category: 'developer',
  },

  // -- Productivity --
  {
    id: 'slack',
    name: 'Slack',
    description: 'Slack workspace integration',
    requiresApiKey: true,
    apiKeyPrompt: 'Enter Slack Bot Token',
    apiKeyPlaceholder: 'xoxb-...',
    apiKeyEnvVar: 'SLACK_BOT_TOKEN',
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-slack'],
      env: { SLACK_BOT_TOKEN: '' },
    },
    category: 'productivity',
  },
  {
    id: 'google-drive',
    name: 'Google Drive',
    description: 'Search and read Google Drive files',
    requiresApiKey: false,
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-gdrive'],
    },
    category: 'productivity',
  },
  {
    id: 'notion',
    name: 'Notion',
    description: 'Notion workspace integration',
    requiresApiKey: true,
    apiKeyPrompt: 'Enter Notion Integration Token',
    apiKeyEnvVar: 'NOTION_API_KEY',
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', 'notion-mcp-server'],
      env: { NOTION_API_KEY: '' },
    },
    category: 'productivity',
  },
  {
    id: 'linear',
    name: 'Linear',
    description: 'Linear project management integration',
    requiresApiKey: true,
    apiKeyPrompt: 'Enter Linear API key',
    apiKeyEnvVar: 'LINEAR_API_KEY',
    config: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', 'linear-mcp-server'],
      env: { LINEAR_API_KEY: '' },
    },
    category: 'productivity',
  },
]

/** Category labels for display */
export const CATEGORY_LABELS: Record<McpServicePreset['category'], string> = {
  filesystem: 'Filesystem',
  memory: 'Memory & Knowledge',
  search: 'Web Search',
  browser: 'Browser & Fetch',
  developer: 'Developer Tools',
  productivity: 'Productivity',
}

/** Get presets grouped by category */
export function getMcpPresetsByCategory(): Record<string, McpServicePreset[]> {
  const result: Record<string, McpServicePreset[]> = {}
  for (const preset of MCP_PRESETS) {
    if (!result[preset.category])
      result[preset.category] = []
    result[preset.category].push(preset)
  }
  return result
}

/** Find an MCP preset by id */
export function findMcpPreset(id: string): McpServicePreset | undefined {
  return MCP_PRESETS.find(p => p.id === id)
}
