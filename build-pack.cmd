@echo off
setlocal EnableExtensions
set "CI=true"

cd /d "%~dp0"

where pnpm >nul 2>nul
if errorlevel 1 (
  echo [ERROR] pnpm was not found. Enable Corepack and install Node.js first.
  exit /b 1
)

echo [1/3] Installing locked dependencies (non-interactive)...
call pnpm install --frozen-lockfile
if errorlevel 1 exit /b %errorlevel%

echo [2/3] Building the unsigned Windows MSI installer...
call pnpm tauri build --bundles msi --config src-tauri\tauri.unsigned.conf.json --no-sign
if errorlevel 1 exit /b %errorlevel%

echo [3/3] Verifying the release executable contains the embedded frontend...
call node scripts\verify-windows-artifact.mjs "%~dp0src-tauri\target\release\cc-launch.exe"
if errorlevel 1 exit /b %errorlevel%

echo.
echo Build completed. The installer is under:
echo src-tauri\target\release\bundle\msi\
echo Do not distribute src-tauri\target\debug\cc-launch.exe; it is a development launcher.
exit /b 0
