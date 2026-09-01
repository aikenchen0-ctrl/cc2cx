import { describe, it, expect } from 'vitest'
import { getAdapter, getAllAdapters, resolveToolType } from '../../src/tools/registry.js'
import { TOOL_TYPES } from '../../src/constants.js'

describe('Tool Registry', () => {
  describe('getAdapter', () => {
    it('should return adapter for each valid tool type', () => {
      for (const type of TOOL_TYPES) {
        const adapter = getAdapter(type)
        expect(adapter).toBeDefined()
        expect(adapter.type).toBe(type)
        expect(adapter.label).toBeTruthy()
        expect(adapter.command).toBeTruthy()
      }
    })

    it('should throw for unknown tool type', () => {
      expect(() => getAdapter('unknown' as any)).toThrow()
    })
  })

  describe('getAllAdapters', () => {
    it('should return all 6 adapters', () => {
      const adapters = getAllAdapters()
      expect(adapters).toHaveLength(6)
    })

    it('should include all tool types', () => {
      const adapters = getAllAdapters()
      const types = adapters.map(a => a.type)
      for (const type of TOOL_TYPES) {
        expect(types).toContain(type)
      }
    })
  })

  describe('resolveToolType', () => {
    it('should resolve exact tool type names', () => {
      expect(resolveToolType('claude-code')).toBe('claude-code')
      expect(resolveToolType('codex')).toBe('codex')
      expect(resolveToolType('gemini-cli')).toBe('gemini-cli')
      expect(resolveToolType('opencode')).toBe('opencode')
      expect(resolveToolType('openclaw')).toBe('openclaw')
      expect(resolveToolType('ccr')).toBe('ccr')
    })

    it('should resolve aliases', () => {
      expect(resolveToolType('cc')).toBe('claude-code')
      expect(resolveToolType('claude')).toBe('claude-code')
      expect(resolveToolType('cx')).toBe('codex')
      expect(resolveToolType('gem')).toBe('gemini-cli')
      expect(resolveToolType('oc')).toBe('opencode')
      expect(resolveToolType('claw')).toBe('openclaw')
      expect(resolveToolType('router')).toBe('ccr')
      expect(resolveToolType('claude-code-router')).toBe('ccr')
    })

    it('should be case-insensitive', () => {
      expect(resolveToolType('CC')).toBe('claude-code')
      expect(resolveToolType('CODEX')).toBe('codex')
      expect(resolveToolType('Claude-Code')).toBe('claude-code')
    })

    it('should trim whitespace', () => {
      expect(resolveToolType(' cc ')).toBe('claude-code')
    })

    it('should throw for unknown alias', () => {
      expect(() => resolveToolType('foobar')).toThrow()
    })
  })
})
