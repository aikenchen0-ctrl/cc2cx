import fs from 'fs-extra'
import { dirname } from 'pathe'
import { commandExists, getPlatform } from 'zcf'
import { x } from 'tinyexec'

import { GEMINI_CONFIG_DIR, GEMINI_SETTINGS_FILE } from '../constants.js'
import { hasHomebrew, hasNpm } from '../utils/platform.js'
import type { ProviderConfig, ToolAdapter, ToolConfigPaths } from './types.js'

export class GeminiCliAdapter implements ToolAdapter {
  readonly type = 'gemini-cli' as const
  readonly label = 'Gemini CLI (Google)'
  readonly command = 'gemini'

  async isInstalled(): Promise<boolean> {
    return commandExists('gemini')
  }

  async install(_silent?: boolean): Promise<void> {
    const platform = getPlatform()

    // Try brew first on macOS
    if (platform === 'macos' && await hasHomebrew()) {
      try {
        await x('brew', ['install', 'gemini-cli'])
        return
      }
      catch {
        // Fallback to npm
      }
    }

    // Fallback: npm global install
    if (await hasNpm()) {
      await x('npm', ['install', '-g', '@google/gemini-cli'])
      return
    }

    throw new Error('Cannot install Gemini CLI: neither brew nor npm is available')
  }

  async detectVersion(): Promise<string | null> {
    try {
      const result = await x('gemini', ['--version'])
      const version = result.stdout.trim()
      return version || null
    }
    catch {
      return null
    }
  }

  async configureProvider(config: ProviderConfig): Promise<void> {
    await fs.ensureDir(dirname(GEMINI_SETTINGS_FILE))

    let existing: Record<string, any> = {}
    if (await fs.pathExists(GEMINI_SETTINGS_FILE)) {
      try {
        existing = await fs.readJson(GEMINI_SETTINGS_FILE)
      }
      catch {
        existing = {}
      }
    }

    // Write API key to settings env section
    existing.env = existing.env ?? {}
    existing.env.GEMINI_API_KEY = config.apiKey

    if (config.apiUrl) {
      existing.env.GEMINI_API_URL = config.apiUrl
    }

    await fs.writeJson(GEMINI_SETTINGS_FILE, existing, { spaces: 2 })
  }

  getConfigPaths(): ToolConfigPaths {
    return {
      configDir: GEMINI_CONFIG_DIR,
      settingsFile: GEMINI_SETTINGS_FILE,
    }
  }
}
