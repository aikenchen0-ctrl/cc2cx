import fs from 'fs-extra'
import { CC_BOOT_DIR, CC_BOOT_CONFIG_FILE } from '../constants.js'

export interface CcBootConfig {
  lang?: string
  ai_output_lang?: string
  installed_tools?: string[]
  provider?: string
  cc_switch_installed?: boolean
  last_init?: string
}

/** Ensure ~/.cc-boot/ exists */
export function ensureConfigDir(): void {
  fs.ensureDirSync(CC_BOOT_DIR)
}

/** Read ~/.cc-boot/config.toml, returns empty object if not found */
export function readConfig(): CcBootConfig {
  ensureConfigDir()
  if (!fs.existsSync(CC_BOOT_CONFIG_FILE)) {
    return {}
  }
  try {
    const content = fs.readFileSync(CC_BOOT_CONFIG_FILE, 'utf-8')
    return parseSimpleToml(content)
  }
  catch {
    return {}
  }
}

/** Write ~/.cc-boot/config.toml */
export function writeConfig(config: CcBootConfig): void {
  ensureConfigDir()
  const content = toSimpleToml(config)
  fs.writeFileSync(CC_BOOT_CONFIG_FILE, content, 'utf-8')
}

/** Merge partial config into existing */
export function updateConfig(partial: Partial<CcBootConfig>): void {
  const existing = readConfig()
  writeConfig({ ...existing, ...partial })
}

// Simple TOML parser/serializer for flat config
function parseSimpleToml(content: string): CcBootConfig {
  const result: Record<string, any> = {}
  for (const line of content.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#'))
      continue
    const eq = trimmed.indexOf('=')
    if (eq === -1)
      continue
    const key = trimmed.slice(0, eq).trim()
    let value = trimmed.slice(eq + 1).trim()
    // Parse value
    if (value.startsWith('"') && value.endsWith('"')) {
      value = value.slice(1, -1)
      result[key] = value
    }
    else if (value.startsWith('[') && value.endsWith(']')) {
      // Simple array of strings
      result[key] = value
        .slice(1, -1)
        .split(',')
        .map(s => s.trim().replace(/^"|"$/g, ''))
        .filter(Boolean)
    }
    else if (value === 'true') {
      result[key] = true
    }
    else if (value === 'false') {
      result[key] = false
    }
    else {
      result[key] = value
    }
  }
  return result as CcBootConfig
}

function toSimpleToml(obj: Record<string, any>): string {
  const lines: string[] = ['# cc-boot configuration', '']
  for (const [key, value] of Object.entries(obj)) {
    if (value === undefined || value === null)
      continue
    if (Array.isArray(value)) {
      lines.push(`${key} = [${value.map(v => `"${v}"`).join(', ')}]`)
    }
    else if (typeof value === 'boolean') {
      lines.push(`${key} = ${value}`)
    }
    else {
      lines.push(`${key} = "${value}"`)
    }
  }
  lines.push('')
  return lines.join('\n')
}
