import fs from 'fs-extra'
import { dirname } from 'pathe'
import { isCodexInstalled, installCodex } from 'zcf'
import { x } from 'tinyexec'

import { CODEX_AUTH_FILE, CODEX_CONFIG_FILE, CODEX_DIR } from '../constants.js'
import type { ProviderConfig, ToolAdapter, ToolConfigPaths } from './types.js'

export class CodexAdapter implements ToolAdapter {
  readonly type = 'codex' as const
  readonly label = 'Codex CLI (OpenAI)'
  readonly command = 'codex'

  async isInstalled(): Promise<boolean> {
    return isCodexInstalled()
  }

  async install(_silent?: boolean): Promise<void> {
    await installCodex()
  }

  async detectVersion(): Promise<string | null> {
    try {
      const result = await x('codex', ['--version'])
      const version = result.stdout.trim()
      return version || null
    }
    catch {
      return null
    }
  }

  async configureProvider(config: ProviderConfig): Promise<void> {
    // Write provider config to ~/.codex/config.yaml
    await fs.ensureDir(dirname(CODEX_CONFIG_FILE))
    const configContent = buildConfigYaml(config)
    await fs.writeFile(CODEX_CONFIG_FILE, configContent, 'utf-8')

    // Write auth to ~/.codex/auth.yaml
    const authContent = buildAuthYaml(config)
    await fs.writeFile(CODEX_AUTH_FILE, authContent, 'utf-8')
  }

  getConfigPaths(): ToolConfigPaths {
    return {
      configDir: CODEX_DIR,
      settingsFile: CODEX_CONFIG_FILE,
    }
  }
}

function buildConfigYaml(config: ProviderConfig): string {
  const lines: string[] = []
  lines.push(`provider: "${config.name}"`)
  if (config.model) {
    lines.push(`model: "${config.model}"`)
  }
  if (config.apiUrl) {
    lines.push(`api_url: "${config.apiUrl}"`)
  }
  lines.push('')
  return lines.join('\n')
}

function buildAuthYaml(config: ProviderConfig): string {
  const lines: string[] = []
  lines.push(`provider: "${config.name}"`)
  lines.push(`api_key: "${config.apiKey}"`)
  lines.push('')
  return lines.join('\n')
}
