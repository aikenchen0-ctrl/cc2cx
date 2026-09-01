import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import fs from 'fs-extra'
import { join } from 'pathe'
import { tmpdir } from 'node:os'

// We test the block writer logic in isolation
describe('Block Writer Logic', () => {
  const BLOCK_START = '<!-- cc-boot:start -->'
  const BLOCK_END = '<!-- cc-boot:end -->'

  function extractBlock(content: string): string | null {
    const startIdx = content.indexOf(BLOCK_START)
    const endIdx = content.indexOf(BLOCK_END)
    if (startIdx === -1 || endIdx === -1 || endIdx <= startIdx)
      return null
    return content.slice(startIdx + BLOCK_START.length, endIdx).trim()
  }

  function insertBlock(content: string, block: string): string {
    const wrapped = `${BLOCK_START}\n${block}\n${BLOCK_END}`
    const startIdx = content.indexOf(BLOCK_START)
    const endIdx = content.indexOf(BLOCK_END)

    if (startIdx !== -1 && endIdx !== -1 && endIdx > startIdx) {
      return content.slice(0, startIdx) + wrapped + content.slice(endIdx + BLOCK_END.length)
    }
    return content.trimEnd() + '\n\n' + wrapped + '\n'
  }

  it('should extract block from content', () => {
    const content = `# My Doc\n\n${BLOCK_START}\nHello World\n${BLOCK_END}\n\n# Other`
    expect(extractBlock(content)).toBe('Hello World')
  })

  it('should return null when no block exists', () => {
    expect(extractBlock('# No block here')).toBeNull()
  })

  it('should insert block into content without existing block', () => {
    const result = insertBlock('# My Doc', 'New Content')
    expect(result).toContain(BLOCK_START)
    expect(result).toContain('New Content')
    expect(result).toContain(BLOCK_END)
  })

  it('should replace existing block', () => {
    const original = `# Doc\n\n${BLOCK_START}\nOld\n${BLOCK_END}\n\n# End`
    const result = insertBlock(original, 'New')
    expect(result).toContain('New')
    expect(result).not.toContain('Old')
    expect(result).toContain('# End')
  })

  it('should preserve content before and after block', () => {
    const original = `# Before\n\n${BLOCK_START}\nOld\n${BLOCK_END}\n\n# After`
    const result = insertBlock(original, 'Updated')
    expect(result).toContain('# Before')
    expect(result).toContain('# After')
    expect(result).toContain('Updated')
  })
})
