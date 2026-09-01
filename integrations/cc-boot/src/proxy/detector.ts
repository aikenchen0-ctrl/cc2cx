import { x } from 'tinyexec'

export interface ProxyInfo {
  type: 'cc-launch' | 'ccr' | 'unknown'
  host: string
  port: number
}

/**
 * Detect active proxy by checking known ports and processes.
 * - CC Switch proxy: localhost ports 49152-65535
 * - CCR (Claude Code Router): typically port 8787 or custom
 */
export async function detectProxy(): Promise<ProxyInfo | null> {
  // Check for CC Switch proxy (high ephemeral ports)
  const ccSwitchProxy = await detectCcSwitchProxy()
  if (ccSwitchProxy) return ccSwitchProxy

  // Check for CCR
  const ccrProxy = await detectCcrProxy()
  if (ccrProxy) return ccrProxy

  return null
}

async function detectCcSwitchProxy(): Promise<ProxyInfo | null> {
  try {
    const result = await x('lsof', ['-i', 'TCP:49152-65535', '-sTCP:LISTEN', '-P', '-n'], {
      throwOnError: false,
    })
    if (result.exitCode !== 0 || !result.stdout) return null

    // Look for CC Switch related processes
    const lines = result.stdout.split('\n')
    for (const line of lines) {
      if (line.toLowerCase().includes('cc-launch') || line.toLowerCase().includes('cclaunch')) {
        const portMatch = line.match(/:(\d+)\s/)
        if (portMatch) {
          return { type: 'cc-launch', host: 'localhost', port: Number.parseInt(portMatch[1], 10) }
        }
      }
    }
  }
  catch {
    // lsof not available or error
  }
  return null
}

async function detectCcrProxy(): Promise<ProxyInfo | null> {
  try {
    const result = await x('pgrep', ['-f', 'ccr'], { throwOnError: false })
    if (result.exitCode !== 0 || !result.stdout.trim()) return null

    // CCR default port
    return { type: 'ccr', host: 'localhost', port: 8787 }
  }
  catch {
    // pgrep not available
  }
  return null
}
