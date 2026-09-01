import fs from 'fs-extra'
import { dirname } from 'pathe'
import { commandExists, getPlatform } from 'zcf'
import { x } from 'tinyexec'

import { OPENCLAW_CONFIG_DIR, OPENCLAW_CONFIG_FILE } from '../constants.js'
import { hasHomebrew, hasPipx } from '../utils/platform.js'
import type { ProviderConfig, ToolAdapter, ToolConfigPaths } from './types.js'

export class OpenClawAdapter implements ToolAdapter {
  readonly type = 'openclaw' as const
  readonly label = 'OpenClaw'
  readonly command = 'openclaw'

  async isInstalled(): Promise<boolean> {
    return commandExists('openclaw')
  }

  async install(_silent?: boolean): Promise<void> {
    const platform = getPlatform()

    // Try brew first on macOS
    if (platform === 'macos' && await hasHomebrew()) {
      try {
        await x('brew', ['install', 'openclaw'])
        return
      }
      catch {
        // Fallback to pipx
      }
    }

    // Fallback: pipx install
    if (await hasPipx()) {
      await x('pipx', ['install', 'openclaw'])
      return
    }

    throw new Error('Cannot install OpenClaw: neither brew nor pipx is available')
  }

  async detectVersion(): Promise<string | null> {
    try {
      const result = await x('openclaw', ['--version'])
      const version = result.stdout.trim()
      return version || null
    }
    catch {
      return null
    }
  }

  async configureProvider(config: ProviderConfig): Promise<void> {
    await fs.ensureDir(dirname(OPENCLAW_CONFIG_FILE))

    let existing: Record<string, any> = {}
    if (await fs.pathExists(OPENCLAW_CONFIG_FILE)) {
      try {
        existing = await fs.readJson(OPENCLAW_CONFIG_FILE)
      }
      catch {
        existing = {}
      }
    }

    existing.provider = {
      name: config.name,
      api_key: config.apiKey,
      ...(config.apiUrl && { api_url: config.apiUrl }),
      ...(config.model && { model: config.model }),
    }

    await fs.writeJson(OPENCLAW_CONFIG_FILE, existing, { spaces: 2 })
  }

  getConfigPaths(): ToolConfigPaths {
    return {
      configDir: OPENCLAW_CONFIG_DIR,
      settingsFile: OPENCLAW_CONFIG_FILE,
    }
  }
}
