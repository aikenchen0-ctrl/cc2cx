import inquirer from 'inquirer'
import { t } from '../i18n/index.js'
import { heading } from '../utils/ui.js'

/**
 * Interactive main menu — routes to init / setup / doctor / handoff / exit.
 */
export async function menu(): Promise<void> {
  heading(t('menu.title'))

  const { action } = await inquirer.prompt<{ action: string }>([
    {
      type: 'list',
      name: 'action',
      message: t('menu.title'),
      choices: [
        { name: t('menu.init'), value: 'init' },
        { name: t('menu.setup'), value: 'setup' },
        { name: t('menu.doctor'), value: 'doctor' },
        { name: t('menu.handoff'), value: 'handoff' },
        new inquirer.Separator(),
        { name: t('menu.exit'), value: 'exit' },
      ],
    },
  ])

  switch (action) {
    case 'init': {
      const { init } = await import('./init.js')
      await init({})
      break
    }
    case 'setup': {
      const { promptAndSetupTool } = await import('./setup-tool.js')
      await promptAndSetupTool()
      break
    }
    case 'doctor': {
      const { doctor } = await import('./doctor.js')
      await doctor()
      break
    }
    case 'handoff': {
      const { handoff } = await import('./handoff.js')
      await handoff({})
      break
    }
    case 'exit':
      return
  }
}
