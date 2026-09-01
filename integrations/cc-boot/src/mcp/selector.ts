import inquirer from 'inquirer'
import { t } from '../i18n/index.js'
import { colors } from '../utils/ui.js'
import type { McpServicePreset } from './presets.js'
import { MCP_PRESETS, CATEGORY_LABELS, getMcpPresetsByCategory } from './presets.js'

export interface McpSelection {
  /** Selected MCP service presets */
  services: McpServicePreset[]
  /** API keys collected for services that require them */
  apiKeys: Record<string, string>
}

/**
 * Interactive multi-select for MCP services.
 * Groups by category, shows descriptions, and collects API keys.
 */
export async function selectMcpServices(): Promise<McpSelection> {
  const byCategory = getMcpPresetsByCategory()
  const choices = buildChoices(byCategory)

  const { serviceIds } = await inquirer.prompt<{ serviceIds: string[] }>([
    {
      type: 'checkbox',
      name: 'serviceIds',
      message: t('mcp.select'),
      choices,
      pageSize: 25,
      loop: false,
    },
  ])

  if (serviceIds.length === 0) {
    return { services: [], apiKeys: {} }
  }

  const selected = serviceIds
    .map(id => MCP_PRESETS.find(p => p.id === id)!)
    .filter(Boolean)

  // Collect API keys for services that require them
  const apiKeys: Record<string, string> = {}
  const needKeys = selected.filter(s => s.requiresApiKey)

  for (const service of needKeys) {
    const prompt = service.apiKeyPrompt || `Enter API key for ${service.name}`
    const { key } = await inquirer.prompt<{ key: string }>([
      {
        type: 'password',
        name: 'key',
        message: prompt,
        mask: '*',
        validate: (v: string) => v.trim().length > 0 || `API key for ${service.name} is required`,
      },
    ])
    apiKeys[service.id] = key.trim()
  }

  return { services: selected, apiKeys }
}

/**
 * Non-interactive MCP service resolution from comma-separated ids.
 */
export function resolveMcpServices(ids: string): McpServicePreset[] {
  return ids
    .split(',')
    .map(id => MCP_PRESETS.find(p => p.id === id.trim()))
    .filter((p): p is McpServicePreset => p !== undefined)
}

// Build checkbox choices grouped by category
function buildChoices(byCategory: Record<string, McpServicePreset[]>) {
  const choices: Array<{ name: string; value: string } | { type: 'separator'; line: string }> = []

  const categoryOrder: McpServicePreset['category'][] = [
    'filesystem',
    'memory',
    'search',
    'browser',
    'developer',
    'productivity',
  ]

  for (const category of categoryOrder) {
    const services = byCategory[category]
    if (!services || services.length === 0)
      continue

    const label = CATEGORY_LABELS[category] || category
    choices.push(new inquirer.Separator(`── ${label} ──`) as any)

    for (const s of services) {
      const keyHint = s.requiresApiKey ? colors.warning(' [key required]') : ''
      const desc = colors.dim(` - ${s.description}`)
      choices.push({
        name: `${s.name}${keyHint}${desc}`,
        value: s.id,
      })
    }
  }

  return choices
}
