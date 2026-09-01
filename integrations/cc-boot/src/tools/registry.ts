import { TOOL_ALIASES, TOOL_TYPES } from '../constants.js'
import type { ToolType } from '../constants.js'
import type { ToolAdapter } from './types.js'
import { ClaudeCodeAdapter } from './claude-code.js'
import { CodexAdapter } from './codex.js'
import { GeminiCliAdapter } from './gemini-cli.js'
import { OpenCodeAdapter } from './opencode.js'
import { OpenClawAdapter } from './openclaw.js'
import { CcrAdapter } from './ccr.js'

const adapterMap: Record<ToolType, ToolAdapter> = {
  'claude-code': new ClaudeCodeAdapter(),
  'codex': new CodexAdapter(),
  'gemini-cli': new GeminiCliAdapter(),
  'opencode': new OpenCodeAdapter(),
  'openclaw': new OpenClawAdapter(),
  'ccr': new CcrAdapter(),
}

/** Get adapter for a specific tool type */
export function getAdapter(type: ToolType): ToolAdapter {
  const adapter = adapterMap[type]
  if (!adapter) {
    throw new Error(`Unknown tool type: ${type}`)
  }
  return adapter
}

/** Get all registered adapters */
export function getAllAdapters(): ToolAdapter[] {
  return TOOL_TYPES.map(type => adapterMap[type])
}

/** Resolve an alias or tool type string to a ToolType */
export function resolveToolType(alias: string): ToolType {
  const normalized = alias.toLowerCase().trim()

  // Direct match
  if (TOOL_TYPES.includes(normalized as ToolType)) {
    return normalized as ToolType
  }

  // Alias match
  const resolved = TOOL_ALIASES[normalized]
  if (resolved) {
    return resolved
  }

  throw new Error(`Unknown tool alias: "${alias}". Valid: ${TOOL_TYPES.join(', ')}`)
}
