import { getPlatform, commandExists } from 'zcf'

export { getPlatform, commandExists }

/** Check if Homebrew is available */
export async function hasHomebrew(): Promise<boolean> {
  return commandExists('brew')
}

/** Check if npm is available */
export async function hasNpm(): Promise<boolean> {
  return commandExists('npm')
}

/** Check if Go is available */
export async function hasGo(): Promise<boolean> {
  return commandExists('go')
}

/** Check if pip/pipx is available */
export async function hasPipx(): Promise<boolean> {
  return commandExists('pipx')
}
