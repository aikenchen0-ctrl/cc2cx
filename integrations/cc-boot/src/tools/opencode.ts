import fs from 'fs-extra'
import { dirname } from 'pathe'
import { commandExists, getPlatform } from 'zcf'
import { x } from 'tinyexec'

import { OPENCODE_CONFIG_DIR, OPENCODE_CONFIG_FILE } from '../constants.js'
import { hasGo, hasHomebrew } from '../utils/platform.js'
import type { ProviderConfig, ToolAdapter, ToolConfigPaths } from './types.js'

export class OpenCodeAdapter implements ToolAdapter {
  readonly type = 'opencode' as const
  readonly label = 'OpenCode'
  readonly command = 'opencode'

  async isInstalled(): Promise<boolean> {
    return commandExists('opencode')
  }

  async install(_silent?: boolean): Promise<void> {
    const platform = getPlatform()

    // Try brew first on macOS
    if (platform === 'macos' && await hasHomebrew()) {
      try {
        await x('brew', ['install', 'opencode'])
        return
      }
      catch {
        // Fallback to go install
      }
    }

    // Fallback: go install
    if (await hasGo()) {
      await x('go', ['install', 'github.com/opencode-ai/opencode@latest'])
      return
    }

    throw new Error('Cannot install OpenCode: neither brew nor go is available')
  }

  async detectVersion(): Promise<string | null> {
    try {
      const result = await x('opencode', ['--version'])
      const version = result.stdout.trim()
      return version || null
    }
    catch {
      return null
    }
  }

  async configureProvider(config: ProviderConfig): Promise<void> {
    await fs.ensureDir(dirname(OPENCODE_CONFIG_FILE))

    // OpenCode uses TOML config
    const lines: string[] = []
    lines.push('[provider]')
    lines.push(`name = "${config.name}"`)
    lines.push(`api_key = "${config.apiKey}"`)
    if (config.apiUrl) {
      lines.push(`api_url = "${config.apiUrl}"`)
    }
    if (config.model) {
      lines.push(`model = "${config.model}"`)
    }
    lines.push('')

    await fs.writeFile(OPENCODE_CONFIG_FILE, lines.join('\n'), 'utf-8')
  }

  getConfigPaths(): ToolConfigPaths {
    return {
      configDir: OPENCODE_CONFIG_DIR,
      settingsFile: OPENCODE_CONFIG_FILE,
    }
  }
}
