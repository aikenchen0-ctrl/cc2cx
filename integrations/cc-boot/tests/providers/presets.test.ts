import { describe, it, expect } from 'vitest'
import { PROVIDER_PRESETS, findProvider, getProvidersByRegion, getProvidersForTool } from '../../src/providers/presets.js'

describe('Provider Presets', () => {
  it('should have at least 30 presets', () => {
    expect(PROVIDER_PRESETS.length).toBeGreaterThanOrEqual(30)
  })

  it('every preset should have required fields', () => {
    for (const p of PROVIDER_PRESETS) {
      expect(p.id).toBeTruthy()
      expect(p.name).toBeTruthy()
      expect(p.authType).toMatch(/^(api_key|auth_token)$/)
      expect(p.supportedTools).toBeDefined()
      expect(p.supportedTools.length).toBeGreaterThan(0)
    }
  })

  it('should have no duplicate IDs', () => {
    const ids = PROVIDER_PRESETS.map(p => p.id)
    const unique = new Set(ids)
    expect(unique.size).toBe(ids.length)
  })

  it('every preset with apiUrl should have a valid URL format (except local/self-hosted)', () => {
    for (const p of PROVIDER_PRESETS) {
      if (p.apiUrl && !p.apiUrl.startsWith('http://localhost') && p.id !== 'custom') {
        expect(p.apiUrl).toMatch(/^https?:\/\//)
      }
    }
  })

  describe('findProvider', () => {
    it('should find existing provider', () => {
      const p = findProvider('302ai')
      expect(p).toBeDefined()
      expect(p?.name).toContain('302')
    })

    it('should return undefined for non-existent', () => {
      expect(findProvider('nonexistent')).toBeUndefined()
    })
  })

  describe('getProvidersByRegion', () => {
    it('should have both CN and Global', () => {
      const byRegion = getProvidersByRegion()
      expect(byRegion.CN.length).toBeGreaterThan(0)
      expect(byRegion.Global.length).toBeGreaterThan(0)
    })

    it('CN providers should have region CN', () => {
      const byRegion = getProvidersByRegion()
      for (const p of byRegion.CN) {
        expect(p.region).toBe('CN')
      }
    })
  })

  describe('getProvidersForTool', () => {
    it('should filter by tool type', () => {
      const ccProviders = getProvidersForTool('claude-code')
      for (const p of ccProviders) {
        expect(p.supportedTools).toContain('claude-code')
      }
    })

    it('should include official Anthropic for claude-code', () => {
      const ccProviders = getProvidersForTool('claude-code')
      const anthropic = ccProviders.find(p => p.id === 'anthropic')
      expect(anthropic).toBeDefined()
    })
  })

  describe('custom provider', () => {
    it('should have a custom provider entry', () => {
      const custom = findProvider('custom')
      expect(custom).toBeDefined()
      expect(custom?.supportedTools.length).toBe(5)
      expect(custom?.apiUrl).toBe('')
    })
  })
})
