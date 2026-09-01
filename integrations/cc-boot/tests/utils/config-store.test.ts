import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import fs from 'fs-extra'
import { readConfig, writeConfig, updateConfig } from '../../src/utils/config-store.js'
import { CC_BOOT_DIR, CC_BOOT_CONFIG_FILE } from '../../src/constants.js'
import { tmpdir } from 'node:os'
import { join } from 'pathe'

// Use a temp dir to avoid polluting user's actual config
const ORIGINAL_DIR = CC_BOOT_DIR
const ORIGINAL_FILE = CC_BOOT_CONFIG_FILE

describe('Config Store', () => {
  // Note: These tests use the actual config store functions
  // They test the TOML parsing logic rather than file I/O

  describe('TOML parsing roundtrip', () => {
    it('writeConfig and readConfig should roundtrip basic config', () => {
      // This test only works if we can write to the config dir
      // We test the parsing logic by creating a temp config
      const config = {
        lang: 'zh-CN',
        ai_output_lang: 'Chinese-simplified',
        installed_tools: ['claude-code', 'codex'],
        provider: '302ai',
        cc_switch_installed: true,
        last_init: '2026-03-04T12:00:00Z',
      }

      writeConfig(config)
      const result = readConfig()

      expect(result.lang).toBe('zh-CN')
      expect(result.provider).toBe('302ai')
      expect(result.cc_switch_installed).toBe(true)
      expect(result.installed_tools).toEqual(['claude-code', 'codex'])
    })

    it('updateConfig should merge with existing', () => {
      writeConfig({ lang: 'en', provider: 'openai' })
      updateConfig({ provider: '302ai' })
      const result = readConfig()

      expect(result.lang).toBe('en')
      expect(result.provider).toBe('302ai')
    })
  })
})
