import inquirer from 'inquirer'
import { t } from '../i18n/index.js'
import { TOOL_LABELS, type ToolType } from '../constants.js'
import { colors } from '../utils/ui.js'
import type { UnifiedProvider } from './types.js'
import { getProvidersByRegion, findProvider, PROVIDER_PRESETS } from './presets.js'

export interface ProviderSelection {
  provider: UnifiedProvider
  apiKey: string
  apiUrl: string
}

/**
 * Interactive provider selector with region grouping.
 * Optionally filter by a specific tool to only show compatible providers.
 */
export async function selectProvider(filterTool?: ToolType): Promise<ProviderSelection> {
  const byRegion = getProvidersByRegion()

  const choices = buildChoices(byRegion, filterTool)

  const { providerId } = await inquirer.prompt<{ providerId: string }>([
    {
      type: 'list',
      name: 'providerId',
      message: t('provider.select'),
      choices,
      loop: false,
      pageSize: 20,
    },
  ])

  let provider = findProvider(providerId)!

  // Custom provider: ask for URL
  let apiUrl = provider.apiUrl
  if (provider.id === 'custom') {
    const { url } = await inquirer.prompt<{ url: string }>([
      {
        type: 'input',
        name: 'url',
        message: t('provider.api_url'),
        validate: (v: string) => v.trim().length > 0 || 'URL is required',
      },
    ])
    apiUrl = url.trim()
  }

  // Ask for API key
  const { apiKey } = await inquirer.prompt<{ apiKey: string }>([
    {
      type: 'password',
      name: 'apiKey',
      message: t('provider.api_key'),
      mask: '*',
      validate: (v: string) => v.trim().length > 0 || 'API key is required',
    },
  ])

  return {
    provider,
    apiKey: apiKey.trim(),
    apiUrl,
  }
}

/**
 * Non-interactive provider resolution by id.
 * Returns the provider preset or undefined.
 */
export function resolveProvider(id: string): UnifiedProvider | undefined {
  return findProvider(id)
}

// Build inquirer choices with region separators
function buildChoices(
  byRegion: Record<'CN' | 'Global', UnifiedProvider[]>,
  filterTool?: ToolType,
) {
  const choices: Array<{ name: string; value: string } | { type: 'separator'; line: string }> = []

  for (const [region, providers] of Object.entries(byRegion) as ['CN' | 'Global', UnifiedProvider[]][]) {
    const filtered = filterTool
      ? providers.filter(p => p.supportedTools.includes(filterTool))
      : providers

    if (filtered.length === 0)
      continue

    const regionLabel = region === 'CN' ? 'China' : 'Global'
    choices.push(new inquirer.Separator(`── ${regionLabel} ──`) as any)

    for (const p of filtered) {
      const toolCount = p.supportedTools.length
      const toolHint = colors.dim(`[${toolCount} tools]`)
      const desc = p.description ? colors.dim(` - ${p.description}`) : ''
      choices.push({
        name: `${p.name} ${toolHint}${desc}`,
        value: p.id,
      })
    }
  }

  return choices
}
