import { heading, success, fail, info, colors, kvLine } from '../utils/ui.js'
import { readConfig, updateConfig, writeConfig } from '../utils/config-store.js'
import type { CcBootConfig } from '../utils/config-store.js'

const VALID_KEYS: Array<keyof CcBootConfig> = [
  'lang',
  'ai_output_lang',
  'installed_tools',
  'provider',
  'cc_switch_installed',
  'last_init',
]

export interface ConfigOptions {
  json?: boolean
  delete?: boolean
}

/**
 * Config command:
 *   cc-boot config              — show all config
 *   cc-boot config get <key>    — get a single key
 *   cc-boot config set <key> <value> — set a key
 *   cc-boot config reset        — clear all config
 */
export async function configCmd(
  action?: string,
  key?: string,
  value?: string,
  options: ConfigOptions = {},
): Promise<void> {
  // No action → show all
  if (!action || action === 'list' || action === 'show') {
    return showAll(options.json)
  }

  if (action === 'get') {
    if (!key) {
      fail('Usage: cc-boot config get <key>')
      printValidKeys()
      process.exit(1)
    }
    return getKey(key)
  }

  if (action === 'set') {
    if (!key || value === undefined) {
      fail('Usage: cc-boot config set <key> <value>')
      printValidKeys()
      process.exit(1)
    }
    return setKey(key, value)
  }

  if (action === 'reset' || action === 'clear') {
    return resetConfig()
  }

  // Unknown action — treat as key name for quick get
  return getKey(action)
}

// ── Helpers ──

function showAll(asJson = false): void {
  const cfg = readConfig()

  if (asJson) {
    console.log(JSON.stringify(cfg, null, 2))
    return
  }

  heading('cc-boot Configuration')

  if (Object.keys(cfg).length === 0) {
    info('No configuration saved yet. Run cc-boot init to get started.')
    return
  }

  console.log()
  for (const [k, v] of Object.entries(cfg)) {
    const display = Array.isArray(v) ? v.join(', ') : String(v)
    kvLine(k, display)
  }

  console.log()
  info(`Config file: ${process.env.HOME}/.cc-boot/config.toml`)
  console.log()
  info(`Edit keys: ${colors.dim('cc-boot config set <key> <value>')}`)
  info(`Valid keys: ${colors.dim(VALID_KEYS.join(', '))}`)
}

function getKey(key: string): void {
  const cfg = readConfig()
  const val = (cfg as Record<string, unknown>)[key]
  if (val === undefined) {
    fail(`Key "${key}" is not set`)
    printValidKeys()
    process.exit(1)
  }
  const display = Array.isArray(val) ? val.join(', ') : String(val)
  console.log(display)
}

function setKey(key: string, value: string): void {
  if (!VALID_KEYS.includes(key as keyof CcBootConfig)) {
    fail(`Unknown config key: "${key}"`)
    printValidKeys()
    process.exit(1)
  }

  // Coerce value type
  let coerced: string | boolean | string[] = value
  if (key === 'cc_switch_installed') {
    coerced = value === 'true' || value === '1'
  }
  else if (key === 'installed_tools') {
    coerced = value.split(',').map(s => s.trim()).filter(Boolean)
  }

  updateConfig({ [key]: coerced } as Partial<CcBootConfig>)
  success(`Set ${colors.bold(key)} = ${colors.dim(Array.isArray(coerced) ? coerced.join(', ') : String(coerced))}`)
}

function resetConfig(): void {
  writeConfig({})
  success('Configuration cleared')
}

function printValidKeys(): void {
  info(`Valid keys: ${VALID_KEYS.join(', ')}`)
}
