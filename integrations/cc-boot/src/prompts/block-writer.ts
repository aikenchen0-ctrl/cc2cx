import fs from 'fs-extra'
import { CLAUDE_MD_FILE, BLOCK_START, BLOCK_END } from '../constants.js'

/**
 * Read the cc-boot managed block from CLAUDE.md.
 * Returns the content between <!-- cc-boot:start --> and <!-- cc-boot:end -->,
 * or null if no block exists.
 */
export async function readBlock(): Promise<string | null> {
  if (!await fs.pathExists(CLAUDE_MD_FILE)) return null

  const content = await fs.readFile(CLAUDE_MD_FILE, 'utf-8')
  const startIdx = content.indexOf(BLOCK_START)
  const endIdx = content.indexOf(BLOCK_END)

  if (startIdx === -1 || endIdx === -1 || endIdx <= startIdx) return null

  return content.slice(startIdx + BLOCK_START.length, endIdx).trim()
}

/**
 * Write or replace the cc-boot managed block in CLAUDE.md.
 * If no block exists, appends it. If a block exists, replaces it.
 */
export async function writeBlock(blockContent: string): Promise<void> {
  const wrapped = `${BLOCK_START}\n${blockContent}\n${BLOCK_END}`

  if (!await fs.pathExists(CLAUDE_MD_FILE)) {
    // Create CLAUDE.md with just the block
    await fs.ensureFile(CLAUDE_MD_FILE)
    await fs.writeFile(CLAUDE_MD_FILE, `${wrapped}\n`, 'utf-8')
    return
  }

  let content = await fs.readFile(CLAUDE_MD_FILE, 'utf-8')
  const startIdx = content.indexOf(BLOCK_START)
  const endIdx = content.indexOf(BLOCK_END)

  if (startIdx !== -1 && endIdx !== -1 && endIdx > startIdx) {
    // Replace existing block
    content = content.slice(0, startIdx) + wrapped + content.slice(endIdx + BLOCK_END.length)
  }
  else {
    // Append block
    content = content.trimEnd() + '\n\n' + wrapped + '\n'
  }

  await fs.writeFile(CLAUDE_MD_FILE, content, 'utf-8')
}

/**
 * Remove the cc-boot managed block from CLAUDE.md.
 */
export async function removeBlock(): Promise<void> {
  if (!await fs.pathExists(CLAUDE_MD_FILE)) return

  let content = await fs.readFile(CLAUDE_MD_FILE, 'utf-8')
  const startIdx = content.indexOf(BLOCK_START)
  const endIdx = content.indexOf(BLOCK_END)

  if (startIdx === -1 || endIdx === -1) return

  content = content.slice(0, startIdx) + content.slice(endIdx + BLOCK_END.length)
  // Clean up multiple blank lines
  content = content.replace(/\n{3,}/g, '\n\n').trim() + '\n'

  await fs.writeFile(CLAUDE_MD_FILE, content, 'utf-8')
}
