import fs from 'fs-extra'
import { join } from 'pathe'
import { CLAUDE_COMMANDS_DIR } from '../constants.js'
import { t } from '../i18n/index.js'
import { spinner, success, info } from '../utils/ui.js'
import type { WorkflowPreset } from './presets.js'

/**
 * Install workflow preset files to ~/.claude/commands/zcf/ namespace.
 */
export async function installWorkflows(presets: WorkflowPreset[]): Promise<void> {
  if (presets.length === 0) {
    info(t('skip'))
    return
  }

  const s = spinner(t('workflow.installing'))
  s.start()

  // Ensure commands directory exists
  await fs.ensureDir(CLAUDE_COMMANDS_DIR)

  for (const preset of presets) {
    for (const [filename, content] of Object.entries(preset.files)) {
      const filePath = join(CLAUDE_COMMANDS_DIR, filename)
      await fs.writeFile(filePath, content, 'utf-8')
    }
  }

  s.stop()
  success(t('workflow.done'))
  info(`  Installed ${presets.length} workflow(s) to ${CLAUDE_COMMANDS_DIR}`)
}
