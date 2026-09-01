import { t } from '../i18n/index.js'
import { heading, info, colors } from '../utils/ui.js'
import { PROVIDER_PRESETS, getProvidersForTool } from '../providers/presets.js'
import { TOOL_TYPES, type ToolType } from '../constants.js'

interface ProvidersOptions {
  tool?: string
  region?: string
  json?: boolean
}

/**
 * List all available API provider presets.
 * Usage: cc-boot providers [--tool claude-code] [--region CN] [--json]
 */
export async function listProviders(options: ProvidersOptions = {}): Promise<void> {
  let providers = PROVIDER_PRESETS

  // Filter by tool
  if (options.tool) {
    // Resolve tool alias to canonical type
    const toolArg = options.tool.toLowerCase().trim()
    const resolvedTool = (TOOL_TYPES as readonly string[]).includes(toolArg)
      ? toolArg
      : null
    if (!resolvedTool) {
      console.error(colors.error(`Unknown tool: "${options.tool}". Valid: ${TOOL_TYPES.join(', ')}`))
      process.exit(1)
    }
    providers = getProvidersForTool(resolvedTool)
  }

  // Filter by region (case-insensitive)
  if (options.region) {
    const region = options.region.toUpperCase()
    providers = providers.filter(p => p.region?.toUpperCase() === region)
  }

  // JSON output
  if (options.json) {
    const out = providers.map(p => ({
      id: p.id,
      name: p.name,
      region: p.region ?? 'Global',
      apiUrl: p.apiUrl,
      defaultModel: p.models?.default ?? null,
      supportedTools: p.supportedTools,
      description: p.description ?? '',
    }))
    console.log(JSON.stringify(out, null, 2))
    return
  }

  heading(t('providers.title'))

  if (providers.length === 0) {
    info(t('providers.none_found'))
    return
  }

  // Group by region
  const byRegion: Record<string, typeof providers> = {}
  for (const p of providers) {
    const region = p.region ?? 'Global'
    if (!byRegion[region]) byRegion[region] = []
    byRegion[region].push(p)
  }

  // Print each region section
  for (const [region, list] of Object.entries(byRegion)) {
    const regionLabel = region === 'CN' ? '🇨🇳 China / 中国区' : '🌐 Global'
    console.log()
    console.log(colors.bold(regionLabel))
    console.log(colors.dim('─'.repeat(72)))

    // Header row
    const ID_W = 20
    const NAME_W = 24
    const MODEL_W = 24
    console.log(
      colors.dim(
        `  ${'ID'.padEnd(ID_W)}${'Name'.padEnd(NAME_W)}${'Default Model'.padEnd(MODEL_W)}Tools`,
      ),
    )
    console.log(colors.dim('  ' + '─'.repeat(70)))

    for (const p of list) {
      const id = colors.primary(p.id.padEnd(ID_W))
      const name = p.name.padEnd(NAME_W)
      const model = colors.dim((p.models?.default ?? '-').padEnd(MODEL_W))
      const tools = p.supportedTools
        .map(tl => shortToolName(tl))
        .join(' ')
      console.log(`  ${id}${name}${model}${tools}`)
    }
  }

  console.log()
  console.log(colors.dim(`${providers.length} providers total`))
  console.log()
  info(t('providers.usage_hint'))
  console.log(colors.dim('  cc-boot init -p <id> -k <api-key>'))
  console.log(colors.dim('  cc-boot providers --tool claude-code'))
  console.log(colors.dim('  cc-boot providers --region CN'))
  console.log(colors.dim('  cc-boot providers --json'))
  console.log()
}

/** Short abbreviation for tool names in provider table */
function shortToolName(tool: string): string {
  const map: Record<string, string> = {
    'claude-code': 'cc',
    'codex': 'cx',
    'gemini-cli': 'gem',
    'opencode': 'oc',
    'openclaw': 'claw',
    'ccr': 'ccr',
  }
  return map[tool] ?? tool
}
