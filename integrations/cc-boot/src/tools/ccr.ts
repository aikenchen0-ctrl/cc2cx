import fs from 'fs-extra'
import { homedir } from 'node:os'
import { join, dirname } from 'pathe'
import { commandExists } from 'zcf'
import { x } from 'tinyexec'

import type { ProviderConfig, ToolAdapter, ToolConfigPaths } from './types.js'

// CCR (Claude Code Router) config paths
const CCR_CONFIG_DIR = join(homedir(), '.claude-code-router')
const CCR_CONFIG_FILE = join(CCR_CONFIG_DIR, 'config.json')

/**
 * Claude Code Router (CCR) adapter
 * https://github.com/musistudio/claude-code-router
 * Install: npm install -g @musistudio/claude-code-router
 * Usage: ccr start — runs a local proxy that Claude Code connects to
 */
export class CcrAdapter implements ToolAdapter {
  readonly type = 'ccr' as const
  readonly label = 'Claude Code Router (CCR)'
  readonly command = 'ccr'

  async isInstalled(): Promise<boolean> {
    return commandExists('ccr')
  }

  async install(_silent?: boolean): Promise<void> {
    // Install via npm globally
    await x('npm', ['install', '-g', '@musistudio/claude-code-router'])
  }

  async detectVersion(): Promise<string | null> {
    try {
      const result = await x('ccr', ['--version'])
      const version = result.stdout.trim()
      return version || null
    }
    catch {
      // Try npm list as fallback
      try {
        const result = await x('npm', ['list', '-g', '@musistudio/claude-code-router', '--depth=0'])
        const match = result.stdout.match(/@musistudio\/claude-code-router@([\d.]+)/)
        return match ? match[1] : null
      }
      catch {
        return null
      }
    }
  }

  async configureProvider(config: ProviderConfig): Promise<void> {
    await fs.ensureDir(dirname(CCR_CONFIG_FILE))

    let existing: Record<string, any> = {}
    if (await fs.pathExists(CCR_CONFIG_FILE)) {
      try {
        existing = await fs.readJson(CCR_CONFIG_FILE)
      }
      catch {
        existing = {}
      }
    }

    // CCR config format: providers array + router settings
    const providers: any[] = existing.providers ?? []

    // Upsert the provider
    const idx = providers.findIndex((p: any) => p.name === config.name)
    const providerEntry: Record<string, any> = {
      name: config.name,
      api_base_url: config.apiUrl || 'https://api.anthropic.com',
      api_key: config.apiKey,
    }
    if (config.model) {
      providerEntry.default_model = config.model
    }

    if (idx >= 0) {
      providers[idx] = providerEntry
    }
    else {
      providers.push(providerEntry)
    }

    existing.providers = providers

    // Set the active provider if not already set
    if (!existing.active_provider) {
      existing.active_provider = config.name
    }

    await fs.writeJson(CCR_CONFIG_FILE, existing, { spaces: 2 })
  }

  getConfigPaths(): ToolConfigPaths {
    return {
      configDir: CCR_CONFIG_DIR,
      settingsFile: CCR_CONFIG_FILE,
    }
  }
}
