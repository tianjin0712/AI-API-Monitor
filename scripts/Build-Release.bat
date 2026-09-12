@echo off
setlocal EnableExtensions

set "SCRIPT_DIR=%~dp0"
set "VERSION=%~1"
set "SKIP_CHECKS="

if /I "%~2"=="-SkipChecks" set "SKIP_CHECKS=-SkipChecks"
if /I "%~2"=="/SkipChecks" set "SKIP_CHECKS=-SkipChecks"

if "%VERSION%"=="" (
  echo Usage: Build-Release.bat ^<version^> [-SkipChecks]
  echo Example: Build-Release.bat 1.0.11
  echo Example: Build-Release.bat 1.0.11 -SkipChecks
  exit /b 2
)

echo Building AI API Monitor release %VERSION%...
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%Build-Release-Windows.ps1" -Version "%VERSION%" %SKIP_CHECKS%
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
  echo Release build failed with exit code %EXIT_CODE%.
  exit /b %EXIT_CODE%
)

echo Release build completed successfully.
exit /b 0
