#!/usr/bin/env node
import cac from 'cac'
import { version } from '../package.json' with { type: 'json' }
import { banner } from './utils/ui.js'
import { initI18n } from './i18n/index.js'
import { registerGlobalErrorHandlers, withErrorHandler } from './utils/error-handler.js'

// Register global handlers first (uncaughtException, unhandledRejection, SIGINT)
registerGlobalErrorHandlers()

await initI18n()

const cli = cac('cc-boot')

// Default: interactive menu
cli
  .command('', 'Interactive menu')
  .action(withErrorHandler('menu', async () => {
    banner()
    const { menu } = await import('./commands/menu.js')
    await menu()
  }))

// init: full initialization
cli
  .command('init', 'Full initialization of AI coding tools')
  .option('--all', 'Install all 5 tools')
  .option('--tools <tools>', 'Comma-separated tool list (cc,cx,gem,oc,claw)')
  .option('-s, --silent', 'Non-interactive mode')
  .option('-p, --provider <provider>', 'API provider preset id or name (e.g. 302ai, deepseek, openai)')
  .option('-k, --api-key <key>', 'API key')
  .option('-u, --api-url <url>', 'Custom API URL')
  .option('-m, --model <model>', 'Override default model for the provider')
  .option('--lang <lang>', 'UI language (zh-CN | en)')
  .option('--ai-lang <lang>', 'AI output language')
  .option('--mcp <services>', 'Comma-separated MCP services')
  .option('--no-cclaunch', 'Skip cc-launch installation')
  .action(withErrorHandler('init', async (options) => {
    banner()
    const { init } = await import('./commands/init.js')
    await init(options)
  }))

// setup: single tool install
cli
  .command('setup <tool>', 'Install and configure a single tool')
  .option('-s, --silent', 'Non-interactive mode')
  .option('-p, --provider <provider>', 'API provider')
  .option('-k, --api-key <key>', 'API key')
  .action(withErrorHandler('setup', async (tool: string, options) => {
    banner()
    const { setupTool } = await import('./commands/setup-tool.js')
    await setupTool(tool, options)
  }))

// doctor: environment diagnostics
cli
  .command('doctor', 'Environment diagnostics')
  .action(withErrorHandler('doctor', async () => {
    banner()
    const { doctor } = await import('./commands/doctor.js')
    await doctor()
  }))

// handoff: export config to cc-launch
cli
  .command('handoff', 'Hand off configuration to cc-launch')
  .option('--install', 'Install cc-launch if not present')
  .action(withErrorHandler('handoff', async (options) => {
    banner()
    const { handoff } = await import('./commands/handoff.js')
    await handoff(options)
  }))

// update: check and apply self-update
cli
  .command('update', 'Check for updates and upgrade cc-boot to the latest version')
  .option('--check', 'Only check for updates without installing')
  .action(withErrorHandler('update', async (options) => {
    banner()
    const { update } = await import('./commands/update.js')
    await update(options)
  }))

// config: view and edit saved configuration
cli
  .command('config [action] [key] [value]', 'View or edit cc-boot configuration')
  .option('--json', 'Output as JSON')
  .action(withErrorHandler('config', async (action: string | undefined, key: string | undefined, value: string | undefined, options) => {
    const { configCmd } = await import('./commands/config.js')
    await configCmd(action, key, value, options)
  }))

// providers: list available API provider presets
cli
  .command('providers', 'List all available API provider presets')
  .option('--tool <tool>', 'Filter by tool (claude-code, codex, gemini-cli, opencode, openclaw, ccr)')
  .option('--region <region>', 'Filter by region (CN or Global)')
  .option('--json', 'Output as JSON')
  .action(withErrorHandler('providers', async (options) => {
    banner()
    const { listProviders } = await import('./commands/providers.js')
    await listProviders(options)
  }))

cli.help()
cli.version(version)

cli.parse()
