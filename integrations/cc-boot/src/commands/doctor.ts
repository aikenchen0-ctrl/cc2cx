import { execSync } from 'node:child_process'
import { t } from '../i18n/index.js'
import { heading, success, fail, info, kvLine, spinner, colors } from '../utils/ui.js'
import { commandExists, hasHomebrew, hasNpm, hasGo, hasPipx } from '../utils/platform.js'
import { TOOL_TYPES, TOOL_LABELS, type ToolType } from '../constants.js'
import { getAdapter } from '../tools/registry.js'
import { isCcSwitchInstalled } from '../handoff/detector.js'
import { detectProxy } from '../proxy/detector.js'

/** Minimum required and recommended Node.js major versions */
const NODE_REQUIRED_MAJOR = 18
const NODE_PREFERRED_MAJOR = 22

interface CheckResult {
  name: string
  ok: boolean
  detail: string
}

/** Get Node.js version string or null if not found */
function getNodeVersion(): string | null {
  try {
    return execSync('node --version', { encoding: 'utf8' }).trim()
  }
  catch {
    return null
  }
}

/** Parse major version number from version string like "v22.1.0" */
function parseMajor(version: string): number {
  const m = version.replace(/^v/, '').split('.')[0]
  return Number.parseInt(m, 10)
}

/** Check Node.js version and return a descriptive CheckResult */
function checkNodeVersion(): CheckResult {
  const version = getNodeVersion()
  if (!version) {
    return { name: 'Node.js', ok: false, detail: 'not found' }
  }

  const major = parseMajor(version)
  if (major < NODE_REQUIRED_MAJOR) {
    return {
      name: 'Node.js',
      ok: false,
      detail: `${version} — requires ≥v${NODE_REQUIRED_MAJOR}, run cc-boot to upgrade`,
    }
  }

  const isPreferred = major >= NODE_PREFERRED_MAJOR
  const hint = isPreferred ? '✓ LTS recommended' : `upgrade to v${NODE_PREFERRED_MAJOR} recommended`
  return {
    name: 'Node.js',
    ok: true,
    detail: `${version} (${hint})`,
  }
}

/**
 * Environment diagnostics — checks all tools, cc-launch, proxy, and system deps.
 */
export async function doctor(): Promise<void> {
  heading(t('doctor.title'))

  const s = spinner(t('doctor.checking'))
  s.start()

  const results: CheckResult[] = []

  // Check Node.js version with detailed output
  results.push(checkNodeVersion())

  // Check other system dependencies
  const systemChecks: Array<{ name: string; check: () => Promise<boolean> }> = [
    { name: 'npm', check: hasNpm },
    { name: 'Homebrew', check: hasHomebrew },
    { name: 'Go', check: hasGo },
    { name: 'pipx', check: hasPipx },
  ]

  for (const { name, check } of systemChecks) {
    const ok = await check()
    results.push({ name, ok, detail: ok ? 'available' : 'not found' })
  }

  // Check each AI coding tool
  for (const toolType of TOOL_TYPES) {
    const result = await checkTool(toolType)
    results.push(result)
  }

  // Check cc-launch
  const ccSwitchOk = await isCcSwitchInstalled()
  results.push({
    name: 'cc-launch',
    ok: ccSwitchOk,
    detail: ccSwitchOk ? t('cclaunch.found') : t('cclaunch.not_found'),
  })

  // Check proxy
  const proxy = await detectProxy()
  results.push({
    name: 'Proxy',
    ok: proxy !== null,
    detail: proxy ? `${proxy.type} @ ${proxy.host}:${proxy.port}` : 'no proxy detected',
  })

  s.stop()

  // Print results
  console.log()
  for (const r of results) {
    const icon = r.ok ? colors.success('✓') : colors.error('✗')
    const detail = r.ok ? colors.success(r.detail) : colors.dim(r.detail)
    console.log(`  ${icon} ${colors.bold(r.name.padEnd(16))} ${detail}`)
  }
  console.log()

  const passed = results.filter(r => r.ok).length
  const total = results.length
  info(`${passed}/${total} checks passed`)
}

async function checkTool(toolType: ToolType): Promise<CheckResult> {
  const label = TOOL_LABELS[toolType]
  try {
    const adapter = getAdapter(toolType)
    const installed = await adapter.isInstalled()
    if (!installed) {
      return { name: label, ok: false, detail: t('tools.not_installed') }
    }
    const version = await adapter.detectVersion()
    return {
      name: label,
      ok: true,
      detail: version ? t('tools.version', { version }) : t('tools.installed'),
    }
  }
  catch {
    return { name: label, ok: false, detail: t('tools.not_installed') }
  }
}
