[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$')]
    [string]$Version,
    [switch]$SkipChecks
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$outputDirectory = Join-Path $projectRoot "release"
$safeVersion = $Version -replace '[<>:"/\\|?*\x00-\x1f]', '-'
$bundleRoot = Join-Path $projectRoot "src-tauri\target\release\bundle"
$nsisDirectory = Join-Path $bundleRoot "nsis"
$msiDirectory = Join-Path $bundleRoot "msi"
$portableDirectory = Join-Path $outputDirectory "AI-API-Monitor-portable"

function Invoke-Pnpm {
    param([string[]]$Arguments)
    & pnpm @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "pnpm $($Arguments -join ' ') failed with exit code $LASTEXITCODE."
    }
}

Push-Location $projectRoot
try {
    Write-Host "Synchronizing manifests to $Version..."
    Invoke-Pnpm @("version:sync", $Version)
    Invoke-Pnpm @("version:check", $Version)

    if (-not $SkipChecks) {
        Invoke-Pnpm @("check")
    }

    Invoke-Pnpm @("build")
    Invoke-Pnpm @("tauri", "build", "--bundles", "nsis,msi")

    $nsisSource = Get-ChildItem -LiteralPath $nsisDirectory -Filter "*.exe" -File |
        Where-Object { $_.Name -notlike "AI API Monitor_x64-setup_$safeVersion.exe" } |
        Select-Object -First 1
    $msiSource = Get-ChildItem -LiteralPath $msiDirectory -Filter "*.msi" -File |
        Where-Object { $_.Name -notlike "AI API Monitor_x64_$safeVersion.msi" } |
        Select-Object -First 1

    if (-not $nsisSource) { throw "NSIS installer was not found in $nsisDirectory." }
    if (-not $msiSource) { throw "MSI installer was not found in $msiDirectory." }

    $nsisTarget = Join-Path $nsisDirectory "AI API Monitor_x64-setup_$safeVersion.exe"
    $msiTarget = Join-Path $msiDirectory "AI API Monitor_x64_$safeVersion.msi"
    if (Test-Path $nsisTarget) { Remove-Item -LiteralPath $nsisTarget -Force }
    if (Test-Path $msiTarget) { Remove-Item -LiteralPath $msiTarget -Force }
    Move-Item -LiteralPath $nsisSource.FullName -Destination $nsisTarget
    Move-Item -LiteralPath $msiSource.FullName -Destination $msiTarget

    if (Test-Path $portableDirectory) { Remove-Item -LiteralPath $portableDirectory -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $portableDirectory | Out-Null
    $appExe = Join-Path $projectRoot "src-tauri\target\release\ai-api-monitor.exe"
    if (-not (Test-Path $appExe)) { throw "Portable executable was not found: $appExe" }
    Copy-Item -LiteralPath $appExe -Destination (Join-Path $portableDirectory "AI API Monitor.exe")
    Copy-Item -LiteralPath (Join-Path $projectRoot "docs\RELEASE.md") -Destination $portableDirectory

    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
    $portableZip = Join-Path $outputDirectory "AI-API-Monitor-portable_$safeVersion.zip"
    if (Test-Path $portableZip) { Remove-Item -LiteralPath $portableZip -Force }
    Compress-Archive -Path (Join-Path $portableDirectory "*") -DestinationPath $portableZip -CompressionLevel Optimal

    Write-Host ""
    Write-Host "Release artifacts:"
    Write-Host "  $nsisTarget"
    Write-Host "  $msiTarget"
    Write-Host "  $portableZip"
}
finally {
    Pop-Location
}
