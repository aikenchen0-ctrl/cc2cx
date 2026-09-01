import type { ToolType } from '../constants.js'

/** Unified provider definition for all AI coding CLI tools */
export interface UnifiedProvider {
  /** Unique identifier, e.g. '302ai', 'openai' */
  id: string
  /** Human-readable display name */
  name: string
  /** Base API URL */
  apiUrl: string
  /** Authentication method */
  authType: 'api_key' | 'auth_token'
  /** Which tools this provider works with */
  supportedTools: ToolType[]
  /** Model overrides per tier */
  models?: {
    default?: string
    haiku?: string
    sonnet?: string
    opus?: string
  }
  /** Short description */
  description?: string
  /** Region tag: 'CN' | 'Global' */
  region?: 'CN' | 'Global'
  /** Whether this is an OpenAI-compatible API */
  openaiCompatible?: boolean
}

/** Region grouping for display */
export type ProviderRegion = 'CN' | 'Global'
