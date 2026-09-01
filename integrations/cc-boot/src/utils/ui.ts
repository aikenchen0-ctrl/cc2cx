import ansis from 'ansis'
import ora from 'ora'

export const colors = {
  primary: ansis.cyan,
  success: ansis.green,
  warning: ansis.yellow,
  error: ansis.red,
  dim: ansis.dim,
  bold: ansis.bold,
  brand: ansis.magenta,
}

export function banner() {
  console.log()
  console.log(colors.brand.bold('  ⚡ cc-boot'))
  console.log(colors.dim('  One-command bootstrap for AI coding CLI tools'))
  console.log()
}

export function heading(text: string) {
  console.log()
  console.log(colors.bold(`▸ ${text}`))
}

export function info(text: string) {
  console.log(colors.dim(`  ${text}`))
}

export function success(text: string) {
  console.log(colors.success(`  ✓ ${text}`))
}

export function warn(text: string) {
  console.log(colors.warning(`  ⚠ ${text}`))
}

export function fail(text: string) {
  console.log(colors.error(`  ✗ ${text}`))
}

export function spinner(text: string) {
  return ora({ text, color: 'cyan', indent: 2 })
}

/** Format a key-value pair for summary display */
export function kvLine(key: string, value: string) {
  console.log(`  ${colors.dim(key + ':')} ${value}`)
}
