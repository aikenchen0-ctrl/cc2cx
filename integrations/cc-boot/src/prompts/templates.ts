/**
 * Default prompt templates for CLAUDE.md injection.
 * These are injected into the cc-boot managed block.
 */

export interface PromptTemplate {
  id: string
  name: string
  content: string
}

/** Build the default cc-boot prompt block content */
export function buildDefaultPrompt(options: {
  provider?: string
  tools?: string[]
  aiLang?: string
}): string {
  const lines: string[] = []

  lines.push('# cc-boot Configuration')
  lines.push('')

  if (options.aiLang) {
    lines.push(`## AI Output Language`)
    lines.push(`Always respond in ${options.aiLang}.`)
    lines.push('')
  }

  if (options.provider) {
    lines.push(`## Provider`)
    lines.push(`Using ${options.provider} as the API provider.`)
    lines.push('')
  }

  if (options.tools && options.tools.length > 0) {
    lines.push(`## Installed Tools`)
    for (const tool of options.tools) {
      lines.push(`- ${tool}`)
    }
    lines.push('')
  }

  lines.push('## Workflows')
  lines.push('Custom workflows are available under `/zcf/` command namespace.')
  lines.push('')

  return lines.join('\n')
}

export const PROMPT_TEMPLATES: PromptTemplate[] = [
  {
    id: 'zh-CN',
    name: 'Chinese Output',
    content: 'Always respond in Chinese-simplified (简体中文).',
  },
  {
    id: 'en',
    name: 'English Output',
    content: 'Always respond in English.',
  },
  {
    id: 'concise',
    name: 'Concise Mode',
    content: 'Be concise. Avoid unnecessary explanations. Focus on actionable output.',
  },
]
