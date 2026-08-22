[CmdletBinding()]
param(
    [switch]$SkipChecks,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot "..\release")
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$outputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

Push-Location $projectRoot
try {
    if (-not $SkipChecks) {
        pnpm check
        if ($LASTEXITCODE -ne 0) { throw "Quality checks failed; packaging stopped." }
    }

    $buildSucceeded = $false
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        Write-Host "Building installer (attempt $attempt of 3)..."
        pnpm tauri build
        if ($LASTEXITCODE -eq 0) {
            $buildSucceeded = $true
            break
        }
        if ($attempt -lt 3) {
            Write-Warning "Build failed. Retrying in 10 seconds; this can recover from a partial WebView2 download."
            Start-Sleep -Seconds 10
        }
    }
    if (-not $buildSucceeded) {
        throw "Installer build failed after 3 attempts. Verify access to https://go.microsoft.com/fwlink/?linkid=2124701, then run this script again."
    }

    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
    $bundleRoot = Join-Path $projectRoot "src-tauri\target\release\bundle\nsis"
    $installer = Get-ChildItem -Path $bundleRoot -Filter "*.exe" -File | Select-Object -First 1
    if (-not $installer) { throw "NSIS installer was not found: $bundleRoot" }
    Copy-Item -LiteralPath $installer.FullName -Destination $outputDirectory -Force

    # Portable output contains the compiled app; frontend assets are embedded by Tauri.
    $portableRoot = Join-Path $outputDirectory "AI-API-Monitor-portable"
    New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null
    $appExe = Get-ChildItem -Path (Join-Path $projectRoot "src-tauri\target\release") -Filter "ai-api-monitor.exe" -File | Select-Object -First 1
    if (-not $appExe) { throw "Compiled application executable was not found." }
    Copy-Item -LiteralPath $appExe.FullName -Destination (Join-Path $portableRoot "AI API Monitor.exe") -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot "docs\RELEASE.md") -Destination $portableRoot -Force
    $portableZip = Join-Path $outputDirectory "AI-API-Monitor-portable.zip"
    if (Test-Path -LiteralPath $portableZip) { Remove-Item -LiteralPath $portableZip -Force }
    Compress-Archive -Path (Join-Path $portableRoot "*") -DestinationPath $portableZip -CompressionLevel Optimal

    Write-Host "Release artifacts created: $outputDirectory"
} finally {
    Pop-Location
}
