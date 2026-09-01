# cc-boot installer for Windows — auto-installs Node.js if missing, then runs cc-boot
# Usage:
#   irm https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.ps1 | iex
#   iex "& { $(irm https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.ps1) } --node-version 20 init --all"

param(
    [int]$NodeVersion = 0,   # --node-version: pin a specific Node.js major (e.g. 20)
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$CcBootArgs
)

$ErrorActionPreference = "Stop"
$REQUIRED_NODE_MAJOR = 18
$PREFERRED_NODE_MAJOR = if ($NodeVersion -gt 0) { $NodeVersion } else { 22 }  # Current LTS
$CC_BOOT_PKG = "@haibane/cc-boot"

function Write-Info  { Write-Host "[cc-boot] $args" -ForegroundColor Cyan }
function Write-Ok    { Write-Host "[cc-boot] $args" -ForegroundColor Green }
function Write-Warn  { Write-Host "[cc-boot] $args" -ForegroundColor Yellow }
function Write-Fail  { Write-Host "[cc-boot] $args" -ForegroundColor Red; exit 1 }

function Write-Banner {
    Write-Host ""
    Write-Host "   +----------------------------------+" -ForegroundColor Cyan
    Write-Host "   |        cc-boot installer         |" -ForegroundColor Cyan
    Write-Host "   |   One-command AI coding setup    |" -ForegroundColor Cyan
    Write-Host "   +----------------------------------+" -ForegroundColor Cyan
    Write-Host ""
}

function Test-Command { param($Name) $null -ne (Get-Command $Name -ErrorAction SilentlyContinue) }

function Get-NodeMajor {
    try {
        $ver = (node --version) -replace '^v', ''
        return [int]($ver.Split('.')[0])
    } catch { return 0 }
}

function Get-NodeVersion {
    try {
        return (node --version) -replace '^v', ''
    } catch { return "unknown" }
}

function Install-Node {
    Write-Info "Installing Node.js v$PREFERRED_NODE_MAJOR LTS..."

    # Try fnm first (fast, cross-platform)
    if (Test-Command "fnm") {
        Write-Info "Using fnm to install Node.js $PREFERRED_NODE_MAJOR..."
        fnm install $PREFERRED_NODE_MAJOR
        fnm use $PREFERRED_NODE_MAJOR
        return
    }

    # Try winget
    if (Test-Command "winget") {
        Write-Info "Using winget to install Node.js LTS..."
        winget install --id OpenJS.NodeJS.LTS --accept-package-agreements --accept-source-agreements
        # Refresh PATH
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
        return
    }

    # Try choco
    if (Test-Command "choco") {
        Write-Info "Using Chocolatey to install Node.js LTS..."
        choco install nodejs-lts -y
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
        return
    }

    # Try scoop
    if (Test-Command "scoop") {
        Write-Info "Using Scoop to install Node.js LTS..."
        scoop install nodejs-lts
        return
    }

    # Fallback: install fnm then use it
    Write-Info "Installing fnm (Fast Node Manager)..."
    if (Test-Command "winget") {
        winget install Schniz.fnm --accept-package-agreements --accept-source-agreements
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
    } else {
        # Direct fnm install via PowerShell
        Invoke-WebRequest -Uri "https://fnm.vercel.app/install" -UseBasicParsing | Select-Object -ExpandProperty Content | Invoke-Expression
    }

    if (Test-Command "fnm") {
        fnm install $PREFERRED_NODE_MAJOR
        fnm use $PREFERRED_NODE_MAJOR
        Write-Ok "fnm installed — add 'fnm env | Out-String | Invoke-Expression' to your PowerShell profile"
    } else {
        Write-Fail "Could not install Node.js automatically. Please install manually: https://nodejs.org"
    }
}

# Main
Write-Banner
Write-Info "Platform: Windows"
Write-Info "Target Node.js: v$PREFERRED_NODE_MAJOR.x"

if (Test-Command "node") {
    $major = Get-NodeMajor
    $ver = Get-NodeVersion
    if ($major -ge $PREFERRED_NODE_MAJOR) {
        Write-Ok "Node.js v$ver OK (meets target v$PREFERRED_NODE_MAJOR)"
    } elseif ($major -ge $REQUIRED_NODE_MAJOR) {
        Write-Warn "Node.js v$ver is below target v$PREFERRED_NODE_MAJOR — installing..."
        Install-Node
    } else {
        Write-Warn "Node.js v$ver is too old (need >= $REQUIRED_NODE_MAJOR)"
        Install-Node
    }
} else {
    Install-Node
}

# Verify
if (-not (Test-Command "node")) {
    Write-Fail "Node.js installation failed. Please install manually: https://nodejs.org"
}

$finalVer = Get-NodeVersion
Write-Ok "Node.js v$finalVer ready"

# Verify npm
if (-not (Test-Command "npm")) {
    Write-Fail "npm not found. Please reinstall Node.js: https://nodejs.org"
}

# Run cc-boot
Write-Info "Launching cc-boot..."
Write-Host ""
if ($CcBootArgs -and $CcBootArgs.Count -gt 0) {
    npx "$CC_BOOT_PKG@latest" @CcBootArgs
} else {
    npx "$CC_BOOT_PKG@latest"
}
