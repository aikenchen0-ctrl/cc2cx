@echo off
setlocal EnableExtensions

cd /d "%~dp0"

where pnpm >nul 2>nul
if errorlevel 1 (
  echo [ERROR] pnpm was not found. Enable Corepack and install Node.js first.
  exit /b 1
)

echo [1/2] Installing locked dependencies...
call pnpm install --frozen-lockfile
if errorlevel 1 exit /b %errorlevel%

echo [2/2] Building the unsigned Windows MSI installer...
call pnpm tauri build --bundles msi --config src-tauri\tauri.unsigned.conf.json
if errorlevel 1 exit /b %errorlevel%

echo.
echo Build completed. The installer is under:
echo src-tauri\target\release\bundle\msi\
exit /b 0
