import inquirer from 'inquirer'
import { t } from '../i18n/index.js'
import { heading, success, fail, info, spinner } from '../utils/ui.js'
import { TOOL_TYPES, TOOL_LABELS, TOOL_ALIASES, type ToolType } from '../constants.js'
import { getAdapter, resolveToolType } from '../tools/registry.js'
import { selectProvider } from '../providers/selector.js'
import type { ProviderConfig } from '../tools/types.js'

export interface SetupToolOptions {
  silent?: boolean
  provider?: string
  apiKey?: string
}

/**
 * Setup a single tool by alias or full name.
 * Installs if not present, then configures provider.
 */
export async function setupTool(tool: string, options: SetupToolOptions): Promise<void> {
  let toolType: ToolType
  try {
    toolType = resolveToolType(tool)
  }
  catch {
    fail(`Unknown tool: "${tool}"`)
    info(`Available tools: ${TOOL_TYPES.join(', ')}`)
    info(`Aliases: ${Object.entries(TOOL_ALIASES).map(([a, t]) => `${a}=${t}`).join(', ')}`)
    return
  }

  heading(`Setup ${TOOL_LABELS[toolType]}`)

  const adapter = getAdapter(toolType)

  // Step 1: Install if not present
  const installed = await adapter.isInstalled()
  if (!installed) {
    const s = spinner(t('tools.installing', { tool: TOOL_LABELS[toolType] }))
    s.start()
    try {
      await adapter.install(options.silent)
      s.stop()
      success(t('tools.install_success', { tool: TOOL_LABELS[toolType] }))
    }
    catch {
      s.stop()
      fail(t('tools.install_fail', { tool: TOOL_LABELS[toolType] }))
      return
    }
  }
  else {
    const version = await adapter.detectVersion()
    success(`${TOOL_LABELS[toolType]} ${version ? `v${version}` : ''} ${t('tools.installed')}`)
  }

  // Step 2: Provider configuration (interactive only)
  if (!options.silent) {
    const selection = await selectProvider(toolType)

    const providerConfig: ProviderConfig = {
      name: selection.provider.name,
      apiKey: selection.apiKey,
      apiUrl: selection.apiUrl,
      authType: selection.provider.authType,
      model: selection.provider.models?.default,
    }

    const s = spinner(t('provider.configuring', { provider: providerConfig.name }))
    s.start()
    await adapter.configureProvider(providerConfig)
    s.stop()
    success(t('provider.done'))
  }

  success(t('done'))
}

/**
 * Interactive prompt to select a tool, then set it up.
 * Used from the menu command.
 */
export async function promptAndSetupTool(): Promise<void> {
  const { tool } = await inquirer.prompt<{ tool: ToolType }>([
    {
      type: 'list',
      name: 'tool',
      message: t('tools.select'),
      choices: TOOL_TYPES.map(tt => ({
        name: TOOL_LABELS[tt],
        value: tt,
      })),
    },
  ])

  await setupTool(tool, {})
}
