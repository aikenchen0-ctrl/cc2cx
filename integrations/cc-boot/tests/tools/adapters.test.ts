import { describe, it, expect } from 'vitest'
import { getAdapter } from '../../src/tools/registry.js'
import { TOOL_TYPES } from '../../src/constants.js'

describe('Tool Adapters', () => {
  describe('getConfigPaths', () => {
    it('each adapter should have valid config paths', () => {
      for (const type of TOOL_TYPES) {
        const adapter = getAdapter(type)
        const paths = adapter.getConfigPaths()
        expect(paths.configDir).toBeTruthy()
        expect(paths.settingsFile).toBeTruthy()
      }
    })
  })

  describe('adapter properties', () => {
    it('claude-code adapter should have correct command', () => {
      const adapter = getAdapter('claude-code')
      expect(adapter.command).toBe('claude')
      expect(adapter.label).toContain('Claude')
    })

    it('codex adapter should have correct command', () => {
      const adapter = getAdapter('codex')
      expect(adapter.command).toBe('codex')
      expect(adapter.label).toContain('Codex')
    })

    it('gemini-cli adapter should have correct command', () => {
      const adapter = getAdapter('gemini-cli')
      expect(adapter.command).toBe('gemini')
      expect(adapter.label).toContain('Gemini')
    })

    it('opencode adapter should have correct command', () => {
      const adapter = getAdapter('opencode')
      expect(adapter.command).toBe('opencode')
    })

    it('openclaw adapter should have correct command', () => {
      const adapter = getAdapter('openclaw')
      expect(adapter.command).toBe('openclaw')
    })
  })

  describe('configureMcp support', () => {
    it('claude-code should support MCP configuration', () => {
      const adapter = getAdapter('claude-code')
      expect(adapter.configureMcp).toBeDefined()
    })
  })
})
