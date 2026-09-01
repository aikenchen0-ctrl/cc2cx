import { describe, it, expect } from 'vitest'
import { MCP_PRESETS, findMcpPreset, getMcpPresetsByCategory } from '../../src/mcp/presets.js'

describe('MCP Presets', () => {
  it('should have at least 10 presets', () => {
    expect(MCP_PRESETS.length).toBeGreaterThanOrEqual(10)
  })

  it('every preset should have required fields', () => {
    for (const p of MCP_PRESETS) {
      expect(p.id).toBeTruthy()
      expect(p.name).toBeTruthy()
      expect(p.description).toBeTruthy()
      expect(typeof p.requiresApiKey).toBe('boolean')
      expect(p.config).toBeDefined()
      expect(p.config.type).toMatch(/^(stdio|sse)$/)
    }
  })

  it('should have no duplicate IDs', () => {
    const ids = MCP_PRESETS.map(p => p.id)
    const unique = new Set(ids)
    expect(unique.size).toBe(ids.length)
  })

  it('presets requiring API key should have apiKeyEnvVar', () => {
    for (const p of MCP_PRESETS) {
      if (p.requiresApiKey) {
        expect(p.apiKeyEnvVar).toBeTruthy()
      }
    }
  })

  it('stdio presets should have command', () => {
    for (const p of MCP_PRESETS) {
      if (p.config.type === 'stdio') {
        expect(p.config.command).toBeTruthy()
      }
    }
  })

  describe('findMcpPreset', () => {
    it('should find existing preset', () => {
      const p = findMcpPreset('memory')
      expect(p).toBeDefined()
      expect(p?.name).toBe('Memory')
    })

    it('should return undefined for non-existent', () => {
      expect(findMcpPreset('nonexistent')).toBeUndefined()
    })
  })

  describe('getMcpPresetsByCategory', () => {
    it('should group presets by category', () => {
      const byCategory = getMcpPresetsByCategory()
      expect(Object.keys(byCategory).length).toBeGreaterThan(0)
      // Each category should have at least 1 preset
      for (const presets of Object.values(byCategory)) {
        expect(presets.length).toBeGreaterThan(0)
      }
    })
  })
})
