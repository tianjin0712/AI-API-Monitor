@echo off
rem ============================================================================
rem  AI API Monitor - one-click release packaging (double-click to run).
rem
rem  The version is auto-detected from the Git tag; no manual version needed.
rem  All arguments are forwarded verbatim to Build-Release.ps1.
rem
rem  Usage:
rem    Build-Release.bat                     auto version -> exe + msi + zip
rem    Build-Release.bat -SkipChecks         skip the quality gate (faster)
rem    Build-Release.bat -Formats msi,zip    only MSI + portable zip
rem    Build-Release.bat -Version 1.0.11     override the detected version
rem    Build-Release.bat -DryRun             preview only, build nothing
rem
rem  NOTE: this file is intentionally ASCII-only. cmd.exe decodes .bat files
rem  using the console code page (936/GBK here), so UTF-8 Chinese text inside a
rem  .bat would be garbled. All Chinese output comes from Build-Release.ps1,
rem  which is saved as UTF-8 with BOM for Windows PowerShell 5.1.
rem ============================================================================
setlocal EnableExtensions

set "SCRIPT_DIR=%~dp0"
set "PS_SCRIPT=%SCRIPT_DIR%Build-Release.ps1"

if not exist "%PS_SCRIPT%" (
  echo [ERROR] Build-Release.ps1 not found: "%PS_SCRIPT%"
  exit /b 2
)

where powershell.exe >nul 2>nul
if errorlevel 1 (
  echo [ERROR] powershell.exe not found; this script requires Windows.
  exit /b 2
)

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%" %*
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
  echo.
  echo [ERROR] Packaging failed with exit code %EXIT_CODE%.
  exit /b %EXIT_CODE%
)

echo.
echo [OK] Packaging finished.
exit /b 0
