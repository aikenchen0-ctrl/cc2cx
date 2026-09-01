import { x } from 'tinyexec'
import { CC_LAUNCH_DEEP_LINK_PREFIX } from '../constants.js'
import type { ProviderConfig, McpServerEntry } from '../tools/types.js'

/**
 * Build a cclaunch:// deep link for provider import.
 * Format: cclaunch://import?type=provider&data=<base64json>
 */
export function buildProviderDeepLink(provider: ProviderConfig): string {
  const payload = {
    name: provider.name,
    apiKey: provider.apiKey,
    apiUrl: provider.apiUrl,
    authType: provider.authType,
    model: provider.model,
  }
  const encoded = Buffer.from(JSON.stringify(payload)).toString('base64url')
  return `${CC_LAUNCH_DEEP_LINK_PREFIX}import?type=provider&data=${encoded}`
}

/**
 * Build a cclaunch:// deep link for MCP server import.
 * Format: cclaunch://import?type=mcp&data=<base64json>
 */
export function buildMcpDeepLink(servers: Record<string, McpServerEntry>): string {
  const encoded = Buffer.from(JSON.stringify(servers)).toString('base64url')
  return `${CC_LAUNCH_DEEP_LINK_PREFIX}import?type=mcp&data=${encoded}`
}

/**
 * Open a deep link URL via the system `open` command (macOS).
 */
export async function openDeepLink(url: string): Promise<void> {
  await x('open', [url])
}
