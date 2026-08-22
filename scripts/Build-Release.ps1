[CmdletBinding()]
param(
    [switch]$SkipChecks,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot "..\release")
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$outputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

# 产品名与架构标记（与 tools/package-windows.mjs、docs/RELEASE.md 保持一致）。
$ProductName = "AI API Monitor"
$Arch = "x64"

# 执行 git 命令并返回单行输出；git 不存在或命令失败时返回 $null，交由调用方回退。
function Get-GitOutput {
    param([string[]]$Arguments)
    try {
        $output = & git @Arguments 2>$null
        if ($LASTEXITCODE -ne 0) { return $null }
        return (($output -join "`n").Trim())
    } catch {
        return $null
    }
}

# 与 tools/package-windows.mjs 的 resolveVersionFromGit 保持一致的语义：
# HEAD 精确 tag > HEAD 可达的最近 tag > $null（调用方回退）。
function Resolve-VersionFromGit {
    $exact = Get-GitOutput @("describe", "--tags", "--exact-match", "HEAD")
    if ($exact) { return $exact }
    $nearest = Get-GitOutput @("describe", "--tags", "--abbrev=0", "HEAD")
    if ($nearest) { return $nearest }
    return $null
}

# 去 v 前缀，得到与 manifest 内部一致的 SemVer（v1.2.3 -> 1.2.3）。
function ConvertTo-NormalizedVersion {
    param([string]$Value)
    return ($Value.Trim() -replace '^[vV]', '')
}

# 仅用于最终文件名的安全处理，不改变版本语义（SemVer 通常不含这些字符，双保险）。
function ConvertTo-SafeFileNamePart {
    param([string]$Value)
    return ($Value -replace '[<>:"/\\|?*\x00-\x1f]', '-')
}

# 回退版本：沿用现有 manifest 机制（package.json），与 package-windows.mjs 的 fallback 一致。
function Get-ManifestFallbackVersion {
    $pkg = Get-Content -LiteralPath (Join-Path $projectRoot "package.json") -Raw | ConvertFrom-Json
    return [string]$pkg.version
}

Push-Location $projectRoot
try {
    # ---- 1. 解析版本：优先 Git Tag，获取失败回退 package.json ----
    $gitTag = Resolve-VersionFromGit
    if ($gitTag) {
        $version = ConvertTo-NormalizedVersion $gitTag
        $versionSource = "Git Tag（$gitTag）"
    } else {
        $version = ConvertTo-NormalizedVersion (Get-ManifestFallbackVersion)
        $versionSource = "fallback（package.json，未检测到 Git Tag）"
    }
    $safeVersion = ConvertTo-SafeFileNamePart $version
    Write-Host "版本来源：$versionSource -> $version"

    # ---- 2. 注入 manifest，保证「应用内部版本」与「产物文件名」同源一致 ----
    # 复用 tools/sync-version.mjs（version-manifests.mjs 的 syncManifestVersions），
    # 同步 Cargo.toml（编译期 CARGO_PKG_VERSION）、tauri.conf.json（app.version / bundle 命名）、
    # package.json 与 Cargo.lock。幂等：manifest 已是目标版本时不改写文件。
    pnpm version:sync $version
    if ($LASTEXITCODE -ne 0) { throw "版本注入失败（pnpm version:sync $version）。" }

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
    $installer = Get-ChildItem -Path $bundleRoot -Filter "*-setup.exe" -File | Select-Object -First 1
    if (-not $installer) { throw "NSIS installer was not found: $bundleRoot" }
    $installerFileName = "${ProductName}_${Arch}-setup_${safeVersion}.exe"
    Copy-Item -LiteralPath $installer.FullName -Destination (Join-Path $outputDirectory $installerFileName) -Force

    # Portable output contains the compiled app; frontend assets are embedded by Tauri.
    $portableRoot = Join-Path $outputDirectory "AI-API-Monitor-portable"
    New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null
    $appExe = Get-ChildItem -Path (Join-Path $projectRoot "src-tauri\target\release") -Filter "ai-api-monitor.exe" -File | Select-Object -First 1
    if (-not $appExe) { throw "Compiled application executable was not found." }
    Copy-Item -LiteralPath $appExe.FullName -Destination (Join-Path $portableRoot "AI API Monitor.exe") -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot "docs\RELEASE.md") -Destination $portableRoot -Force
    $portableZip = Join-Path $outputDirectory "AI-API-Monitor-portable_${safeVersion}.zip"
    if (Test-Path -LiteralPath $portableZip) { Remove-Item -LiteralPath $portableZip -Force }
    Compress-Archive -Path (Join-Path $portableRoot "*") -DestinationPath $portableZip -CompressionLevel Optimal

    Write-Host "Release artifacts created: $outputDirectory"
} finally {
    Pop-Location
}
