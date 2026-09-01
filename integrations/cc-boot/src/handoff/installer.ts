import { x } from 'tinyexec'
import { t } from '../i18n/index.js'
import { CC_LAUNCH_BREW_CASK } from '../constants.js'
import { spinner, success, fail, warn, info } from '../utils/ui.js'
import { hasHomebrew, commandExists } from '../utils/platform.js'

// Upstream: farion1231/cc-switch (cross-platform)
const CC_LAUNCH_REPO = 'farion1231/cc-switch'
const CC_LAUNCH_RELEASES_API = `https://api.github.com/repos/${CC_LAUNCH_REPO}/releases/latest`
const CC_LAUNCH_RELEASES_URL = `https://github.com/${CC_LAUNCH_REPO}/releases/latest`

/**
 * Install cc-launch with cross-platform support:
 * - macOS: Homebrew cask → browser fallback
 * - Linux: deb (apt) → rpm (dnf/yum) → AppImage fallback
 * - Windows: browser fallback (xdg-open / powershell start)
 */
export async function installCcSwitch(): Promise<void> {
  const platform = process.platform

  if (platform === 'darwin') {
    await installMacos()
  }
  else if (platform === 'linux') {
    await installLinux()
  }
  else if (platform === 'win32') {
    await installWindows()
  }
  else {
    warn(`Unsupported platform: ${platform}. Download cc-launch from: ${CC_LAUNCH_RELEASES_URL}`)
  }
}

// ── macOS ──────────────────────────────────────────────────────────────────

async function installMacos(): Promise<void> {
  const s = spinner(t('cclaunch.installing'))
  s.start()

  if (await hasHomebrew()) {
    try {
      await x('brew', ['install', '--cask', CC_LAUNCH_BREW_CASK])
      s.stop()
      success(t('cclaunch.done'))
      return
    }
    catch {
      s.stop()
      warn('Homebrew install failed, opening download page...')
    }
  }
  else {
    s.stop()
  }

  // Fallback: open download URL in browser
  await openBrowser(CC_LAUNCH_RELEASES_URL)
  warn(`Please download cc-launch from: ${CC_LAUNCH_RELEASES_URL}`)
}

// ── Linux ──────────────────────────────────────────────────────────────────

async function installLinux(): Promise<void> {
  // Fetch latest release info and find Linux assets
  info('Fetching latest cc-launch release info...')

  let assetUrl: string | null = null
  let assetType: 'deb' | 'rpm' | 'appimage' | null = null

  try {
    const releaseInfo = await fetchLatestRelease()
    const hasDeb = await commandExists('apt-get') || await commandExists('apt')
    const hasRpm = await commandExists('dnf') || await commandExists('yum')

    // Prefer deb → rpm → AppImage based on available package manager
    if (hasDeb) {
      assetUrl = findLinuxAsset(releaseInfo, '.deb')
      if (assetUrl) assetType = 'deb'
    }
    if (!assetUrl && hasRpm) {
      assetUrl = findLinuxAsset(releaseInfo, '.rpm')
      if (assetUrl) assetType = 'rpm'
    }
    if (!assetUrl) {
      assetUrl = findLinuxAsset(releaseInfo, '.AppImage')
      if (assetUrl) assetType = 'appimage'
    }
  }
  catch (err) {
    warn(`Could not fetch release info: ${err}`)
  }

  if (!assetUrl || !assetType) {
    warn(`Could not determine Linux package. Opening download page...`)
    await openBrowser(CC_LAUNCH_RELEASES_URL)
    warn(`Please download cc-launch from: ${CC_LAUNCH_RELEASES_URL}`)
    return
  }

  const s = spinner(t('cclaunch.installing'))
  s.start()

  try {
    if (assetType === 'deb') {
      await downloadAndInstallDeb(assetUrl)
    }
    else if (assetType === 'rpm') {
      await downloadAndInstallRpm(assetUrl)
    }
    else if (assetType === 'appimage') {
      await downloadAndInstallAppImage(assetUrl)
    }
    s.stop()
    success(t('cclaunch.done'))
  }
  catch (err) {
    s.stop()
    warn(`Auto-install failed: ${err}`)
    warn(`Please download manually from: ${CC_LAUNCH_RELEASES_URL}`)
  }
}

// ── Windows ────────────────────────────────────────────────────────────────

async function installWindows(): Promise<void> {
  // Try winget first
  if (await commandExists('winget')) {
    const s = spinner(t('cclaunch.installing'))
    s.start()
    try {
      await x('winget', ['install', '--id', 'farion1231.CCSwitch', '--silent', '--accept-package-agreements', '--accept-source-agreements'])
      s.stop()
      success(t('cclaunch.done'))
      return
    }
    catch {
      s.stop()
      warn('winget install failed, opening download page...')
    }
  }

  // Fallback: open browser
  await openBrowser(CC_LAUNCH_RELEASES_URL)
  warn(`Please download cc-launch from: ${CC_LAUNCH_RELEASES_URL}`)
}

// ── Helpers ────────────────────────────────────────────────────────────────

interface GithubRelease {
  tag_name: string
  assets: Array<{ name: string, browser_download_url: string }>
}

async function fetchLatestRelease(): Promise<GithubRelease> {
  // Use node's built-in fetch (Node 18+)
  const res = await fetch(CC_LAUNCH_RELEASES_API, {
    headers: { 'User-Agent': 'cc-boot-installer' },
  })
  if (!res.ok) {
    throw new Error(`GitHub API returned ${res.status}`)
  }
  return res.json() as Promise<GithubRelease>
}

function findLinuxAsset(release: GithubRelease, ext: string): string | null {
  const arch = process.arch === 'arm64' ? 'arm64' : 'x86_64'
  // Try arch-specific first
  let asset = release.assets.find(a =>
    a.name.toLowerCase().includes('linux')
    && a.name.includes(arch)
    && a.name.toLowerCase().endsWith(ext.toLowerCase())
    && !a.name.endsWith('.sig')
  )
  // Fallback to any Linux asset with matching ext
  if (!asset) {
    asset = release.assets.find(a =>
      a.name.toLowerCase().includes('linux')
      && a.name.toLowerCase().endsWith(ext.toLowerCase())
      && !a.name.endsWith('.sig')
    )
  }
  return asset?.browser_download_url ?? null
}

async function downloadAndInstallDeb(url: string): Promise<void> {
  const tmpFile = `/tmp/cc-launch-installer.deb`
  await downloadFile(url, tmpFile)
  // Try without sudo first, then with sudo
  try {
    await x('dpkg', ['-i', tmpFile])
  }
  catch {
    await x('sudo', ['dpkg', '-i', tmpFile])
  }
}

async function downloadAndInstallRpm(url: string): Promise<void> {
  const tmpFile = `/tmp/cc-launch-installer.rpm`
  await downloadFile(url, tmpFile)
  const hasDnf = await commandExists('dnf')
  const pm = hasDnf ? 'dnf' : 'yum'
  try {
    await x(pm, ['localinstall', '-y', tmpFile])
  }
  catch {
    await x('sudo', [pm, 'localinstall', '-y', tmpFile])
  }
}

async function downloadAndInstallAppImage(url: string): Promise<void> {
  const installDir = `${process.env.HOME ?? '/root'}/.local/bin`
  const destFile = `${installDir}/cc-launch`
  // Ensure directory exists
  await x('mkdir', ['-p', installDir])
  await downloadFile(url, destFile)
  await x('chmod', ['+x', destFile])
  info(`cc-launch AppImage installed to ${destFile}`)
  info(`Make sure ${installDir} is in your PATH`)
}

async function downloadFile(url: string, dest: string): Promise<void> {
  // Use wget or curl
  const hasWget = await commandExists('wget')
  if (hasWget) {
    await x('wget', ['-q', '-O', dest, url])
  }
  else {
    const hasCurl = await commandExists('curl')
    if (hasCurl) {
      await x('curl', ['-sL', '-o', dest, url])
    }
    else {
      // Node fetch fallback
      const res = await fetch(url, { headers: { 'User-Agent': 'cc-boot-installer' } })
      if (!res.ok) throw new Error(`Download failed: ${res.status}`)
      const buf = await res.arrayBuffer()
      const { writeFileSync } = await import('node:fs')
      writeFileSync(dest, Buffer.from(buf))
    }
  }
}

async function openBrowser(url: string): Promise<void> {
  const platform = process.platform
  try {
    if (platform === 'darwin') {
      await x('open', [url])
    }
    else if (platform === 'win32') {
      await x('powershell', ['-Command', `Start-Process '${url}'`])
    }
    else {
      // Linux: xdg-open
      await x('xdg-open', [url])
    }
  }
  catch {
    // Silently ignore if browser open fails
  }
}
