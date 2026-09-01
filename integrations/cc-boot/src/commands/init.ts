import inquirer from 'inquirer'
import { t, setLanguage } from '../i18n/index.js'
import { heading, success, fail, info, spinner, kvLine } from '../utils/ui.js'
import { updateConfig } from '../utils/config-store.js'
import { TOOL_TYPES, TOOL_LABELS, type ToolType } from '../constants.js'
import { getAdapter, getAllAdapters, resolveToolType } from '../tools/registry.js'
import { selectProvider } from '../providers/selector.js'
import { selectMcpServices } from '../mcp/selector.js'
import { writeMcpForTool } from '../mcp/writer.js'
import { isCcSwitchInstalled } from '../handoff/detector.js'
import { installCcSwitch } from '../handoff/installer.js'
import { buildProviderDeepLink, openDeepLink } from '../handoff/deep-link.js'
import { WORKFLOW_PRESETS } from '../workflows/presets.js'
import { installWorkflows } from '../workflows/installer.js'
import { writeBlock } from '../prompts/block-writer.js'
import { buildDefaultPrompt } from '../prompts/templates.js'
import type { ProviderConfig, McpServerEntry } from '../tools/types.js'

export interface InitOptions {
  all?: boolean
  tools?: string
  silent?: boolean
  provider?: string
  apiKey?: string
  apiUrl?: string
  model?: string
  lang?: string
  aiLang?: string
  mcp?: string
  cclaunch?: boolean  // --no-cclaunch sets this to false
}

/**
 * Full 8-step init flow:
 * 1. Language → 2. Tool selection → 3. Install → 4. Provider
 * → 5. MCP → 6. Workflows → 7. cc-launch → 8. Summary
 */
export async function init(options: InitOptions): Promise<void> {
  heading(t('init.title'))

  // ── Step 1: Language ──
  const lang = await stepLanguage(options)

  // ── Step 2: Tool selection ──
  const selectedTools = await stepSelectTools(options)
  if (selectedTools.length === 0) {
    fail('No tools selected')
    return
  }

  // ── Step 3: Install missing tools ──
  await stepInstallTools(selectedTools, options.silent)

  // ── Step 4: Provider configuration ──
  const providerConfig = await stepProvider(selectedTools, options)

  // ── Step 5: MCP configuration ──
  const mcpResult = await stepMcp(selectedTools, options)

  // ── Step 6: Workflow installation ──
  const installedWorkflows = await stepWorkflows(selectedTools, options)

  // ── Step 7: cc-launch handoff ──
  await stepCcSwitch(options, providerConfig)

  // ── Step 8: Summary ──
  await stepSummary({
    lang,
    tools: selectedTools,
    provider: providerConfig?.name,
    model: providerConfig?.model,
    mcpCount: mcpResult ? Object.keys(mcpResult).length : 0,
    workflowCount: installedWorkflows,
  })
}

// ── Step 1: Language ──

async function stepLanguage(options: InitOptions): Promise<string> {
  let lang = options.lang

  if (!lang && !options.silent) {
    const { selectedLang } = await inquirer.prompt<{ selectedLang: string }>([
      {
        type: 'list',
        name: 'selectedLang',
        message: t('lang.select'),
        choices: [
          { name: '简体中文', value: 'zh-CN' },
          { name: 'English', value: 'en' },
        ],
        default: 'zh-CN',
      },
    ])
    lang = selectedLang
  }

  lang = lang || 'zh-CN'
  setLanguage(lang)
  updateConfig({ lang })

  // AI output language
  let aiLang = options.aiLang
  if (!aiLang && !options.silent) {
    const { selectedAiLang } = await inquirer.prompt<{ selectedAiLang: string }>([
      {
        type: 'list',
        name: 'selectedAiLang',
        message: t('lang.ai_output'),
        choices: [
          { name: '简体中文', value: 'Chinese-simplified' },
          { name: 'English', value: 'English' },
          { name: '日本語', value: 'Japanese' },
        ],
        default: lang === 'zh-CN' ? 'Chinese-simplified' : 'English',
      },
    ])
    aiLang = selectedAiLang
  }

  if (aiLang) {
    updateConfig({ ai_output_lang: aiLang })
  }

  return lang
}

// ── Step 2: Tool selection ──

async function stepSelectTools(options: InitOptions): Promise<ToolType[]> {
  if (options.all) {
    return [...TOOL_TYPES]
  }

  if (options.tools) {
    const aliases = options.tools.split(',').map(s => s.trim())
    const resolved: ToolType[] = []
    for (const alias of aliases) {
      try {
        resolved.push(resolveToolType(alias))
      }
      catch {
        fail(`Unknown tool alias: "${alias}"`)
      }
    }
    return resolved
  }

  if (options.silent) {
    // Silent mode with no tools specified: default to claude-code
    return ['claude-code']
  }

  // Interactive: checkbox selection
  const adapters = getAllAdapters()
  const choices = await Promise.all(
    adapters.map(async (adapter) => {
      const installed = await adapter.isInstalled()
      const suffix = installed ? ` (${t('tools.installed')})` : ''
      return {
        name: `${adapter.label}${suffix}`,
        value: adapter.type,
        checked: installed, // pre-check installed tools
      }
    }),
  )

  const { selectedTools } = await inquirer.prompt<{ selectedTools: ToolType[] }>([
    {
      type: 'checkbox',
      name: 'selectedTools',
      message: t('tools.select'),
      choices,
    },
  ])

  return selectedTools
}

// ── Step 3: Install missing tools ──

async function stepInstallTools(tools: ToolType[], silent?: boolean): Promise<void> {
  for (const toolType of tools) {
    const adapter = getAdapter(toolType)
    const installed = await adapter.isInstalled()
    if (installed) {
      const version = await adapter.detectVersion()
      success(`${adapter.label} ${version ? `v${version}` : ''} ${t('tools.installed')}`)
      continue
    }

    const s = spinner(t('tools.installing', { tool: adapter.label }))
    s.start()
    try {
      await adapter.install(silent)
      s.stop()
      success(t('tools.install_success', { tool: adapter.label }))
    }
    catch {
      s.stop()
      fail(t('tools.install_fail', { tool: adapter.label }))
    }
  }
}

// ── Step 4: Provider ──

async function stepProvider(
  tools: ToolType[],
  options: InitOptions,
): Promise<ProviderConfig | null> {
  heading(t('provider.select'))

  let providerResult: ProviderConfig | null = null

  if (options.silent && options.provider && options.apiKey) {
    // Non-interactive: use CLI flags
    const { findProvider, findProviderFuzzy } = await import('../providers/presets.js')
    const preset = findProvider(options.provider) ?? findProviderFuzzy(options.provider)
    if (preset) {
      providerResult = {
        name: preset.name,
        apiKey: options.apiKey,
        apiUrl: options.apiUrl || preset.apiUrl,
        authType: preset.authType,
        model: options.model || preset.models?.default,
      }
    }
    else {
      fail(`Unknown provider: "${options.provider}". Use --help to see provider ids.`)
    }
  }
  else if (!options.silent) {
    // Interactive: use selector
    const selection = await selectProvider()
    providerResult = {
      name: selection.provider.name,
      apiKey: selection.apiKey,
      apiUrl: selection.apiUrl,
      authType: selection.provider.authType,
    }
  }

  if (!providerResult) {
    info(t('skip'))
    return null
  }

  // Apply provider to all selected tools
  for (const toolType of tools) {
    const adapter = getAdapter(toolType)
    const s = spinner(t('provider.configuring', { provider: providerResult.name }))
    s.start()
    try {
      await adapter.configureProvider(providerResult)
      s.stop()
    }
    catch {
      s.stop()
      fail(`Failed to configure ${adapter.label}`)
    }
  }

  updateConfig({ provider: providerResult.name })
  success(t('provider.done'))
  return providerResult
}

// ── Step 5: MCP ──

async function stepMcp(
  tools: ToolType[],
  options: InitOptions,
): Promise<Record<string, McpServerEntry> | null> {
  heading(t('mcp.select'))

  let mcpResult: Record<string, McpServerEntry> | null = null

  if (options.silent && options.mcp) {
    // Non-interactive: resolve from CLI flag
    const { resolveMcpServices } = await import('../mcp/selector.js')
    const { buildMcpServers } = await import('../mcp/writer.js')
    const services = resolveMcpServices(options.mcp)
    mcpResult = buildMcpServers(services, {})
  }
  else if (!options.silent) {
    // Interactive: use selector
    const selection = await selectMcpServices()
    if (selection.services.length > 0) {
      const { buildMcpServers } = await import('../mcp/writer.js')
      mcpResult = buildMcpServers(selection.services, selection.apiKeys)
    }
  }

  if (!mcpResult || Object.keys(mcpResult).length === 0) {
    info(t('skip'))
    return null
  }

  // Apply MCP to all tools
  for (const toolType of tools) {
    try {
      await writeMcpForTool(toolType, mcpResult)
    }
    catch {
      fail(`Failed to configure MCP for ${TOOL_LABELS[toolType]}`)
    }
  }

  success(t('mcp.done'))
  return mcpResult
}

// ── Step 6: Workflows ──

async function stepWorkflows(
  tools: ToolType[],
  options: InitOptions,
): Promise<number> {
  // Workflows only apply to claude-code and codex
  const hasClaudeOrCodex = tools.some(t => t === 'claude-code' || t === 'codex')
  if (!hasClaudeOrCodex) return 0

  heading(t('workflow.select'))

  let selectedPresets = WORKFLOW_PRESETS

  if (!options.silent) {
    const { chosen } = await inquirer.prompt<{ chosen: string[] }>([
      {
        type: 'checkbox',
        name: 'chosen',
        message: t('workflow.select'),
        choices: WORKFLOW_PRESETS.map(p => ({
          name: `${p.name} — ${p.description}`,
          value: p.id,
          checked: true,
        })),
      },
    ])
    selectedPresets = WORKFLOW_PRESETS.filter(p => chosen.includes(p.id))
  }

  await installWorkflows(selectedPresets)
  return selectedPresets.length
}

// ── Step 7: cc-launch ──

async function stepCcSwitch(
  options: InitOptions,
  providerConfig: ProviderConfig | null,
): Promise<void> {
  // Skip if --no-cclaunch
  if (options.cclaunch === false) return

  heading(t('cclaunch.detect'))

  const installed = await isCcSwitchInstalled()

  if (installed) {
    success(t('cclaunch.found'))
    // Hand off provider config via deep link
    if (providerConfig) {
      const link = buildProviderDeepLink(providerConfig)
      try {
        await openDeepLink(link)
        success(t('cclaunch.done'))
      }
      catch {
        info('cc-launch deep link failed — configure manually')
      }
    }
    return
  }

  if (options.silent) return

  const { shouldInstall } = await inquirer.prompt<{ shouldInstall: boolean }>([
    {
      type: 'confirm',
      name: 'shouldInstall',
      message: t('cclaunch.install_prompt'),
      default: true,
    },
  ])

  if (shouldInstall) {
    await installCcSwitch()
    updateConfig({ cc_switch_installed: true })
  }
}

// ── Step 8: Summary ──

async function stepSummary(summary: {
  lang: string
  tools: ToolType[]
  provider?: string
  model?: string
  mcpCount: number
  workflowCount: number
}): Promise<void> {
  // Write prompt block to CLAUDE.md
  const promptContent = buildDefaultPrompt({
    provider: summary.provider,
    tools: summary.tools,
  })
  await writeBlock(promptContent)

  // Save config
  updateConfig({
    installed_tools: summary.tools,
    last_init: new Date().toISOString(),
  })

  heading(t('init.summary'))
  kvLine('Language', summary.lang)
  kvLine('Tools', summary.tools.map(t => TOOL_LABELS[t]).join(', '))
  kvLine('Provider', summary.provider || '-')
  if (summary.model) {
    kvLine('Model', summary.model)
  }
  kvLine('MCP Services', String(summary.mcpCount))
  kvLine('Workflows', String(summary.workflowCount))
  console.log()
  success(t('init.complete'))
}
