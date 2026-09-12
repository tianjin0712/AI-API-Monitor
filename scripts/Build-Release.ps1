<#
.SYNOPSIS
    AI API Monitor Windows 一键发布打包脚本。

.DESCRIPTION
    自动检测版本号，构建并收集三种发布产物，文件名遵循 docs/RELEASE.md 的既定约定：

        release/AI API Monitor_x64-setup_<version>.exe   NSIS 安装包
        release/AI API Monitor_x64_<version>.msi         MSI 安装包
        release/AI-API-Monitor-portable_<version>.zip    便携版（含 RELEASE.md）
        release/AI-API-Monitor-portable/                 便携版暂存目录

    版本来源优先级（与 tools/package-windows.mjs 的 resolveVersionFromGit 完全一致）：

        1. -Version 显式指定
        2. HEAD 上的精确 Git Tag          （git describe --tags --exact-match）
        3. HEAD 可达的最近 Git Tag        （git describe --tags --abbrev=0）
        4. 回退到 package.json 的 version

    版本号会注入受控 manifest（package.json / tauri.conf.json / Cargo.toml / Cargo.lock），
    保证「产物文件名」与「应用内部版本」同源一致。该注入是幂等的：manifest 已是目标
    版本时不会改写文件。

.PARAMETER Version
    手动指定版本号（可带 v 前缀，如 v1.0.11）。留空则自动检测。

.PARAMETER Formats
    要产出的产物类型，可多选：msi / exe / zip。默认三者全出。

.PARAMETER OutputDirectory
    产物输出目录，默认 <仓库根>\release。

.PARAMETER SkipChecks
    跳过 pnpm check 质量门禁（typecheck / test / rust fmt+check+clippy+test）。

.PARAMETER DryRun
    只解析版本并打印构建计划，不注入 manifest、不构建、不产出任何文件。

.EXAMPLE
    .\scripts\Build-Release.ps1
    自动检测版本并产出 exe + msi + zip。

.EXAMPLE
    .\scripts\Build-Release.ps1 -SkipChecks
    同上，但跳过质量门禁（构建更快）。

.EXAMPLE
    .\scripts\Build-Release.ps1 -Formats msi,zip
    只产出 MSI 与便携版 ZIP（不构建 NSIS）。

.EXAMPLE
    .\scripts\Build-Release.ps1 -Version 1.0.11 -DryRun
    预览「若以 1.0.11 打包」的版本来源与产物清单，不做任何改动。
#>
[CmdletBinding()]
param(
    [string]$Version,
    [ValidateSet("msi", "exe", "zip")]
    [string[]]$Formats = @("msi", "exe", "zip"),
    [string]$OutputDirectory,
    [switch]$SkipChecks,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# ---------------------------------------------------------------------------
# 常量：与 tools/package-windows.mjs、docs/RELEASE.md 保持一致。
# ---------------------------------------------------------------------------
$ProductName = "AI API Monitor"
$Arch = "x64"
$PortableDirName = "AI-API-Monitor-portable"

# 与 tools/version-manifests.mjs 的 SEMVER 完全一致（含可选的预发布/构建元数据）。
$SemVerPattern = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$'

# 受控 manifest：版本注入与「构建后不得被改写」校验都针对这四个文件。
$ManifestPaths = @(
    "package.json"
    "src-tauri/tauri.conf.json"
    "src-tauri/Cargo.toml"
    "src-tauri/Cargo.lock"
)

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $ProjectRoot "release"
}
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

$ReleaseDir = Join-Path $ProjectRoot "src-tauri\target\release"
$NsisDir = Join-Path $ReleaseDir "bundle\nsis"
$MsiDir = Join-Path $ReleaseDir "bundle\msi"
$PortableExe = Join-Path $ReleaseDir "ai-api-monitor.exe"
$ReleaseNotes = Join-Path $ProjectRoot "docs\RELEASE.md"

# ---------------------------------------------------------------------------
# 辅助函数
# ---------------------------------------------------------------------------

# 执行 git 并返回单行输出；git 不存在或命令失败时返回 $null（交由调用方回退）。
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

function Fail {
    param([string]$Message)
    Write-Host ""
    Write-Host "✗ $Message" -ForegroundColor Red
    exit 1
}

# 执行 pnpm；失败即终止，避免把上一步的失败当成成功继续打包。
function Invoke-Pnpm {
    param([string[]]$Arguments)
    Write-Host "`n> pnpm $($Arguments -join ' ')" -ForegroundColor Cyan
    & pnpm @Arguments
    if ($LASTEXITCODE -ne 0) {
        Fail "pnpm $($Arguments -join ' ') 失败（退出码 $LASTEXITCODE）。"
    }
}

# 仅用于文件名，不改变版本语义（SemVer 通常不含这些字符，双保险）。
function ConvertTo-SafeFileNamePart {
    param([string]$Value)
    return ($Value -replace '[<>:"/\\|?*\x00-\x1f]', '-')
}

# 在目录中按「文件名包含目标版本号」定位产物。
#
# bundle 目录会跨版本累积，例如 nsis/ 下可能同时存在 0.1.0 / 1.0.4 / 1.0.6 / 1.0.7 /
# 1.0.8 / 1.0.10 五个残留安装包。若只取「第一个」或只按扩展名匹配，就会把旧版本的
# 安装包当成新版本发出去。因此这里强制按版本号匹配，并且匹配不到时直接失败并列出
# 目录现有内容，绝不静默回退到其它版本。
function Get-VersionedFile {
    param(
        [string]$Directory,
        [string]$Extension,
        [string]$VersionTag,
        [string]$Description,
        [string]$NamePattern = "*"
    )
    if (-not (Test-Path -LiteralPath $Directory)) {
        Fail "未找到 $Description 目录：$Directory"
    }
    $all = @(Get-ChildItem -LiteralPath $Directory -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Extension -eq $Extension -and $_.Name -like $NamePattern })
    if ($all.Count -eq 0) {
        Fail "在 $Directory 中未找到 $Description（$NamePattern$Extension）。"
    }
    $matched = @($all |
        Where-Object { $_.Name -like "*$VersionTag*" } |
        Sort-Object LastWriteTime -Descending)
    if ($matched.Count -eq 0) {
        $found = ($all | Select-Object -ExpandProperty Name | Sort-Object) -join ", "
        Fail "在 $Directory 中未找到版本 $VersionTag 的 $Description。该目录现有：$found"
    }
    return $matched[0]
}

function Format-Size {
    param([long]$Bytes)
    if ($Bytes -ge 1MB) { return ("{0:N1} MB" -f ($Bytes / 1MB)) }
    if ($Bytes -ge 1KB) { return ("{0:N1} KB" -f ($Bytes / 1KB)) }
    return "$Bytes B"
}

# ---------------------------------------------------------------------------
# 1. 解析版本号
# ---------------------------------------------------------------------------
Push-Location $ProjectRoot
try {
    $releaseVersion = $null
    $releaseVersionSource = $null

    if (-not [string]::IsNullOrWhiteSpace($Version)) {
        $releaseVersion = $Version.Trim() -replace '^[vV]', ''
        $releaseVersionSource = "命令行 -Version"
    } else {
        $exact = Get-GitOutput @("describe", "--tags", "--exact-match", "HEAD")
        if ($exact) {
            $releaseVersion = $exact -replace '^[vV]', ''
            $releaseVersionSource = "Git Tag（精确匹配 $exact）"
        } else {
            $nearest = Get-GitOutput @("describe", "--tags", "--abbrev=0", "HEAD")
            if ($nearest) {
                $releaseVersion = $nearest -replace '^[vV]', ''
                $releaseVersionSource = "Git Tag（最近可达 $nearest，HEAD 不在 Tag 上）"
            } else {
                $packageJson = Get-Content -LiteralPath (Join-Path $ProjectRoot "package.json") -Raw | ConvertFrom-Json
                $releaseVersion = [string]$packageJson.version
                $releaseVersionSource = "回退 package.json（未检测到 Git Tag）"
            }
        }
    }

    if ([string]::IsNullOrWhiteSpace($releaseVersion)) {
        Fail "未能解析版本号；请用 -Version 显式指定。"
    }
    if ($releaseVersion -notmatch $SemVerPattern) {
        Fail "版本号 '$releaseVersion' 不是有效的 SemVer（来自：$releaseVersionSource）。"
    }
    $safeVersion = ConvertTo-SafeFileNamePart $releaseVersion

    # -----------------------------------------------------------------------
    # 2. 归一化产物类型，推导 tauri --bundles 取值
    # -----------------------------------------------------------------------
    $selected = @($Formats | ForEach-Object { $_.ToLowerInvariant() } | Select-Object -Unique)
    $wantExe = $selected -contains "exe"
    $wantMsi = $selected -contains "msi"
    $wantZip = $selected -contains "zip"

    $bundles = @()
    if ($wantExe) { $bundles += "nsis" }
    if ($wantMsi) { $bundles += "msi" }

    # -----------------------------------------------------------------------
    # 3. 打印计划（DryRun 到此为止）
    # -----------------------------------------------------------------------
    Write-Host ""
    Write-Host "AI API Monitor 发布打包" -ForegroundColor Green
    Write-Host ("=" * 60)
    Write-Host "  版本来源 : $releaseVersionSource"
    Write-Host "  版本号   : $releaseVersion"
    Write-Host "  产物类型 : $($selected -join ', ')"
    if ($bundles.Count -gt 0) {
        Write-Host "  Tauri    : pnpm tauri build --bundles $($bundles -join ',') --ci"
    } else {
        Write-Host "  Tauri    : pnpm tauri build --no-bundle --ci   (仅需编译产物，不打安装包)"
    }
    Write-Host "  质量门禁 : $(if ($SkipChecks) { '跳过（-SkipChecks）' } else { '执行 pnpm check' })"
    Write-Host "  输出目录 : $OutputDirectory"
    Write-Host "  预期产物 :"
    if ($wantExe) { Write-Host "    - ${ProductName}_${Arch}-setup_$safeVersion.exe" }
    if ($wantMsi) { Write-Host "    - ${ProductName}_${Arch}_$safeVersion.msi" }
    if ($wantZip) { Write-Host "    - ${PortableDirName}_$safeVersion.zip" }
    Write-Host ("=" * 60)

    if ($DryRun) {
        Write-Host "`n[DryRun] 仅预览，未做任何改动。" -ForegroundColor Yellow
        return
    }

    # -----------------------------------------------------------------------
    # 4. 前置检查
    # -----------------------------------------------------------------------
    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
        Fail "未找到 pnpm；请先安装 pnpm（package.json 声明 packageManager=pnpm@11.21.0）。"
    }

    # 记录构建前 manifest 的脏状态，构建后用于校验「构建过程不得改写版本」。
    $manifestsDirtyBefore = Get-GitOutput (@("status", "--porcelain", "--") + $ManifestPaths)

    # -----------------------------------------------------------------------
    # 5. 注入 manifest 版本（幂等），并断言一致性
    # -----------------------------------------------------------------------
    Invoke-Pnpm @("version:sync", $releaseVersion)
    Invoke-Pnpm @("version:check", $releaseVersion)

    # -----------------------------------------------------------------------
    # 6. 质量门禁
    # -----------------------------------------------------------------------
    if (-not $SkipChecks) {
        Invoke-Pnpm @("check")
    }

    # -----------------------------------------------------------------------
    # 7. 构建：Tauri 会先跑 beforeBuildCommand（pnpm version:check && pnpm build）
    # -----------------------------------------------------------------------
    if ($bundles.Count -gt 0) {
        Invoke-Pnpm @("tauri", "build", "--bundles", ($bundles -join ","), "--ci")
    } else {
        Invoke-Pnpm @("tauri", "build", "--no-bundle", "--ci")
    }

    # -----------------------------------------------------------------------
    # 8. 收集产物到输出目录（复制而非移动，保留 target 下的原始构建输出）
    # -----------------------------------------------------------------------
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $produced = @()

    if ($wantExe) {
        $nsisSource = Get-VersionedFile -Directory $NsisDir -Extension ".exe" -NamePattern "*-setup.exe" `
            -VersionTag $safeVersion -Description "NSIS 安装包"
        $exeTarget = Join-Path $OutputDirectory "${ProductName}_${Arch}-setup_${safeVersion}.exe"
        Copy-Item -LiteralPath $nsisSource.FullName -Destination $exeTarget -Force
        $produced += $exeTarget
    }

    if ($wantMsi) {
        $msiSource = Get-VersionedFile -Directory $MsiDir -Extension ".msi" `
            -VersionTag $safeVersion -Description "MSI 安装包"
        $msiTarget = Join-Path $OutputDirectory "${ProductName}_${Arch}_${safeVersion}.msi"
        Copy-Item -LiteralPath $msiSource.FullName -Destination $msiTarget -Force
        $produced += $msiTarget
    }

    if ($wantZip) {
        if (-not (Test-Path -LiteralPath $PortableExe)) {
            Fail "未找到便携版可执行文件：$PortableExe"
        }
        if (-not (Test-Path -LiteralPath $ReleaseNotes)) {
            Fail "未找到便携版随附的发布说明：$ReleaseNotes"
        }

        # 便携版内容与 CI（.github/workflows/release.yml）保持一致：
        # 编译产物重命名为 "AI API Monitor.exe"，并随附 RELEASE.md。
        $portableDir = Join-Path $OutputDirectory $PortableDirName
        if (Test-Path -LiteralPath $portableDir) {
            Remove-Item -LiteralPath $portableDir -Recurse -Force
        }
        New-Item -ItemType Directory -Force -Path $portableDir | Out-Null
        Copy-Item -LiteralPath $PortableExe -Destination (Join-Path $portableDir "AI API Monitor.exe") -Force
        Copy-Item -LiteralPath $ReleaseNotes -Destination $portableDir -Force

        $zipPath = Join-Path $OutputDirectory "${PortableDirName}_$safeVersion.zip"
        if (Test-Path -LiteralPath $zipPath) {
            Remove-Item -LiteralPath $zipPath -Force
        }
        # 用 "*" 展开，使压缩包根目录直接是 exe 与 RELEASE.md（与 CI 行为一致）。
        Compress-Archive -Path (Join-Path $portableDir "*") -DestinationPath $zipPath -CompressionLevel Optimal
        $produced += $zipPath
    }

    # -----------------------------------------------------------------------
    # 9. 校验构建过程没有改写受控 manifest（与 CI 的守卫同义）
    # -----------------------------------------------------------------------
    $manifestsDirtyAfter = Get-GitOutput (@("status", "--porcelain", "--") + $ManifestPaths)
    if ($manifestsDirtyAfter -ne $manifestsDirtyBefore) {
        Write-Warning "本次打包改写了受控 manifest："
        Write-Warning "  构建前：$(if ($manifestsDirtyBefore) { $manifestsDirtyBefore } else { '(干净)' })"
        Write-Warning "  构建后：$(if ($manifestsDirtyAfter) { $manifestsDirtyAfter } else { '(干净)' })"
        Write-Warning "这通常说明 release commit 里的版本号与 Tag 不一致。请把 manifest 变更提交到 release commit，"
        Write-Warning "而不是靠打包脚本回写版本（CI 会因此失败）。"
    }

    # -----------------------------------------------------------------------
    # 10. 汇总
    # -----------------------------------------------------------------------
    Write-Host ""
    Write-Host "打包完成，产物如下：" -ForegroundColor Green
    foreach ($item in $produced) {
        $size = Format-Size (Get-Item -LiteralPath $item).Length
        Write-Host ("  {0,-52} {1,10}" -f (Split-Path $item -Leaf), $size)
    }
    Write-Host ""
    Write-Host "输出目录：$OutputDirectory"
} finally {
    Pop-Location
}
