import { describe, it, expect } from 'vitest'
import { buildProviderDeepLink, buildMcpDeepLink } from '../../src/handoff/deep-link.js'
import type { ProviderConfig, McpServerEntry } from '../../src/tools/types.js'

describe('Deep Link Construction', () => {
  describe('buildProviderDeepLink', () => {
    it('should create valid cclaunch:// URL', () => {
      const config: ProviderConfig = {
        name: 'Test Provider',
        apiKey: 'sk-test-key',
        apiUrl: 'https://api.test.com',
        authType: 'api_key',
      }
      const url = buildProviderDeepLink(config)
      expect(url).toMatch(/^cclaunch:\/\/import\?type=provider&data=/)
    })

    it('should encode payload as base64url', () => {
      const config: ProviderConfig = {
        name: 'Test',
        apiKey: 'key',
        apiUrl: 'https://api.test.com',
        authType: 'api_key',
      }
      const url = buildProviderDeepLink(config)
      const data = url.split('data=')[1]
      // base64url should not contain +, /, =
      expect(data).not.toMatch(/[+/=]/)
      // Should be decodable
      const decoded = JSON.parse(Buffer.from(data, 'base64url').toString())
      expect(decoded.name).toBe('Test')
      expect(decoded.apiKey).toBe('key')
    })
  })

  describe('buildMcpDeepLink', () => {
    it('should create valid cclaunch:// URL for MCP', () => {
      const servers: Record<string, McpServerEntry> = {
        memory: { type: 'stdio', command: 'npx', args: ['-y', '@mcp/server-memory'] },
      }
      const url = buildMcpDeepLink(servers)
      expect(url).toMatch(/^cclaunch:\/\/import\?type=mcp&data=/)
    })

    it('should encode multiple servers', () => {
      const servers: Record<string, McpServerEntry> = {
        memory: { type: 'stdio', command: 'npx', args: ['-y', 'mem'] },
        fetch: { type: 'stdio', command: 'npx', args: ['-y', 'fetch'] },
      }
      const url = buildMcpDeepLink(servers)
      const data = url.split('data=')[1]
      const decoded = JSON.parse(Buffer.from(data, 'base64url').toString())
      expect(Object.keys(decoded)).toHaveLength(2)
      expect(decoded.memory).toBeDefined()
      expect(decoded.fetch).toBeDefined()
    })
  })
})
