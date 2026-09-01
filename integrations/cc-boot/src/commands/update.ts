import { x } from 'tinyexec'
import { t } from '../i18n/index.js'
import { heading, success, fail, info, spinner, colors, warn } from '../utils/ui.js'
import { version as currentVersion } from '../../package.json' with { type: 'json' }

const PACKAGE_NAME = '@haibane/cc-boot'

/**
 * Compare semver strings. Returns true if `latest` is newer than `current`.
 */
function isNewer(current: string, latest: string): boolean {
  const parse = (v: string) => v.replace(/^v/, '').split('.').map(Number)
  const [cMaj, cMin, cPat] = parse(current)
  const [lMaj, lMin, lPat] = parse(latest)
  if (lMaj !== cMaj) return lMaj > cMaj
  if (lMin !== cMin) return lMin > cMin
  return lPat > cPat
}

/**
 * Fetch the latest published version from npm registry.
 */
async function fetchLatestVersion(): Promise<string | null> {
  try {
    const result = await x('npm', ['view', PACKAGE_NAME, 'version', '--json'], {
      throwOnError: false,
    })
    const raw = result.stdout.trim()
    // npm returns a JSON string like "\"0.1.1\""
    return raw.replace(/^"|"$/g, '')
  }
  catch {
    return null
  }
}

/**
 * `cc-boot update` — check for a newer version and self-update via npm.
 */
export async function update(options: { check?: boolean } = {}): Promise<void> {
  heading('Update cc-boot')

  info(`Current version: ${colors.bold(currentVersion)}`)

  const s = spinner('Checking npm registry for latest version…')
  s.start()

  const latest = await fetchLatestVersion()
  s.stop()

  if (!latest) {
    fail('Could not reach npm registry. Check your network/proxy and try again.')
    process.exit(1)
  }

  info(`Latest version:  ${colors.bold(latest)}`)
  console.log()

  if (!isNewer(currentVersion, latest)) {
    success(`You are already on the latest version (${currentVersion}). Nothing to do.`)
    return
  }

  warn(`New version available: ${colors.bold(currentVersion)} → ${colors.bold(latest)}`)

  if (options.check) {
    // --check flag: just report, don't install
    console.log()
    info(`Run ${colors.primary('cc-boot update')} (without --check) to upgrade.`)
    return
  }

  // Determine how cc-boot was installed
  const isGlobal = await detectGlobalInstall()

  console.log()
  const us = spinner(`Updating to ${latest}…`)
  us.start()

  try {
    if (isGlobal) {
      await x('npm', ['install', '-g', `${PACKAGE_NAME}@${latest}`], { throwOnError: true })
    }
    else {
      // Fallback: update in place (npx scenario)
      await x('npm', ['install', '-g', `${PACKAGE_NAME}@${latest}`], { throwOnError: true })
    }
    us.stop()
    console.log()
    success(`Updated successfully to ${latest}! 🎉`)
    info('Restart your terminal or run cc-boot again to use the new version.')
  }
  catch (err: unknown) {
    us.stop()
    const message = err instanceof Error ? err.message : String(err)
    fail(`Update failed: ${message}`)
    console.log()
    info(`Try manually: ${colors.primary(`npm install -g ${PACKAGE_NAME}@${latest}`)}`)
    process.exit(1)
  }
}

/**
 * Detect whether the currently-running binary was installed globally.
 * Heuristic: check if `npm list -g` lists our package.
 */
async function detectGlobalInstall(): Promise<boolean> {
  try {
    const result = await x('npm', ['list', '-g', '--depth=0', '--json'], { throwOnError: false })
    const json = JSON.parse(result.stdout)
    return !!json?.dependencies?.[PACKAGE_NAME]
  }
  catch {
    return true // assume global if we can't tell
  }
}
