import { x } from 'tinyexec'
import { t } from '../i18n/index.js'
import { spinner, success, warn, fail } from '../utils/ui.js'
import { commandExists } from '../utils/platform.js'

/**
 * Check if CCR (Claude Code Router) is installed,
 * and offer to set it up as a proxy fallback when CC Switch is unavailable.
 */
export async function setupCcrFallback(): Promise<void> {
  const hasCcr = await commandExists('ccr')
  if (!hasCcr) {
    warn('CCR (Claude Code Router) is not installed.')
    warn('Install it via: npm install -g @anthropic/ccr')
    return
  }

  const s = spinner('Starting CCR...')
  s.start()

  try {
    // Verify CCR is runnable
    const result = await x('ccr', ['--version'], { throwOnError: false })
    s.stop()
    if (result.exitCode === 0) {
      success(`CCR available: ${result.stdout.trim()}`)
    }
    else {
      fail('CCR found but failed version check')
    }
  }
  catch {
    s.stop()
    fail('Failed to verify CCR installation')
  }
}
