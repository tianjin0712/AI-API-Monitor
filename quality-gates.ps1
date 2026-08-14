<#
.SYNOPSIS
  AI API Monitor 质量门禁一键脚本（Windows / 13600KF + RTX 4070 台式机优先）

.DESCRIPTION
  按顺序执行与 CI（.github/workflows/quality.yml）完全一致的四步质量门禁：

    pnpm install --frozen-lockfile
    pnpm check                      # typecheck + vitest + cargo fmt/clippy/test
    pnpm build                      # tsc + vite build
    pnpm security:audit             # pnpm audit --prod + cargo audit

  自动检测并安装缺失工具（Node / pnpm / Rust / cargo-audit），
  每一步独立记录成败，最后输出汇总表并返回退出码（0 = 全部通过）。

.PARAMETER SkipToolInstall
  跳过工具自动安装；缺失的工具直接按失败处理并输出安装指引。

.PARAMETER SkipAudit
  跳过 security:audit 步骤（例如暂时无网络访问 GitHub/RustSec 时）。

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File quality-gates.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File quality-gates.ps1 -SkipAudit
#>

[CmdletBinding()]
param(
    [switch]$SkipToolInstall,
    [switch]$SkipAudit
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# 中文输出与 UTF-8
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

# 非交互模式：允许 pnpm 在无 TTY 时清理旧 modules 目录，与 CI 行为一致
$env:CI = 'true'

$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $RepoRoot

$script:GateResults = [System.Collections.Generic.List[object]]::new()
$script:Skipped     = [System.Collections.Generic.List[string]]::new()

function Write-Step([string]$title) {
    Write-Host ""
    Write-Host ("=" * 72) -ForegroundColor Cyan
    Write-Host ("  " + $title) -ForegroundColor Cyan
    Write-Host ("=" * 72) -ForegroundColor Cyan
}

function Get-ToolPath([string]$name) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($null -ne $cmd) { return @($cmd)[0].Source }
    return $null
}

function Refresh-Path {
    # 让当前会话看到刚安装的工具（安装器只改注册表 PATH）
    $machine = [System.Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [System.Environment]::GetEnvironmentVariable('Path', 'User')
    $env:Path = ($machine + ';' + $user).TrimEnd(';')
}

function Invoke-Gate {
    param(
        [string]$Name,
        [scriptblock]$Block
    )
    Write-Step $Name
    $code = 0
    try {
        # Out-Host 流式输出到控制台并吞掉管道，确保本函数只返回退出码
        & $Block | Out-Host
        $code = $LASTEXITCODE
        if ($null -eq $code) { $code = 0 }
    }
    catch {
        Write-Host ("  [ERROR] " + $_.Exception.Message) -ForegroundColor Red
        $code = 1
    }
    $script:GateResults.Add([pscustomobject]@{ Name = $Name; Code = $code })
    if ($code -ne 0) {
        Write-Host ("  [FAILED] " + $Name + " (exit " + $code + ")") -ForegroundColor Red
    }
    else {
        Write-Host ("  [OK] " + $Name) -ForegroundColor Green
    }
    return [int]$code
}

function Get-LastCode {
    if ($script:GateResults.Count -eq 0) { return 0 }
    return ($script:GateResults | Select-Object -Last 1).Code
}

# ---------------------------------------------------------------------------
# 1. 工具检测
# ---------------------------------------------------------------------------
Write-Step "工具检测 (Tool detection)"

$nodeExe   = Get-ToolPath 'node'
$npmExe    = Get-ToolPath 'npm'
$pnpmExe   = Get-ToolPath 'pnpm'
$cargoExe  = Get-ToolPath 'cargo'
$rustupExe = Get-ToolPath 'rustup'
$wingetExe = Get-ToolPath 'winget'

if ($nodeExe) {
    $nodeVer = (& node --version) -replace '^v', ''
    Write-Host ("  node        : " + $nodeVer) -ForegroundColor Gray
    # engines: ^20.19.0 || ^22.12.0 || >=24.0.0
    $major = [int](($nodeVer -split '\.')[0])
    $minor = [int](($nodeVer -split '\.')[1])
    $ok = ($major -eq 20 -and $minor -ge 19) -or ($major -eq 22 -and $minor -ge 12) -or ($major -ge 24)
    if (-not $ok) {
        Write-Host "  [WARN] Node 版本不满足 package.json engines（^20.19 || ^22.12 || >=24），建议使用 Node 22 LTS（与 CI 一致）。" -ForegroundColor Yellow
    }
}
else { Write-Host "  node        : MISSING" -ForegroundColor Yellow }

if ($pnpmExe) { Write-Host ("  pnpm        : " + (& pnpm --version)) -ForegroundColor Gray }
else { Write-Host "  pnpm        : MISSING" -ForegroundColor Yellow }

if ($cargoExe) { Write-Host ("  cargo       : " + (& cargo --version)) -ForegroundColor Gray }
else { Write-Host "  cargo       : MISSING" -ForegroundColor Yellow }

if (Get-ToolPath 'cargo-audit') {
    Write-Host ("  cargo-audit : " + (& cargo-audit --version)) -ForegroundColor Gray
}
else { Write-Host "  cargo-audit : MISSING（security:audit 需要，将自动安装）" -ForegroundColor Yellow }

if (-not $wingetExe) { Write-Host "  winget      : MISSING（工具自动安装将不可用）" -ForegroundColor Yellow }

# ---------------------------------------------------------------------------
# 2. 缺失工具自动安装
# ---------------------------------------------------------------------------
$toolFailed = $false

if (-not $SkipToolInstall) {
    if (-not $nodeExe) {
        Write-Step "安装 Node.js（winget: OpenJS.NodeJS.LTS）"
        if ($wingetExe) {
            winget install OpenJS.NodeJS.LTS --accept-package-agreements --accept-source-agreements --silent
            Refresh-Path
            $nodeExe = Get-ToolPath 'node'
        }
        if (-not $nodeExe) {
            Write-Host "  [ERROR] 未能自动安装 Node.js。请手动安装 Node 22 LTS 后重试：https://nodejs.org/" -ForegroundColor Red
            $toolFailed = $true
        }
    }

    if (-not $pnpmExe) {
        Write-Step "安装 pnpm 11.21.0"
        # 首选 corepack（可精确锁定 packageManager 版本），退回 npm 全局安装
        $corepack = Get-ToolPath 'corepack'
        if ($corepack) {
            try {
                & $corepack enable
                & $corepack prepare pnpm@11.21.0 --activate
                Refresh-Path
            }
            catch { Write-Host "  corepack 方式失败，尝试 npm 全局安装..." -ForegroundColor Gray }
        }
        $pnpmExe = Get-ToolPath 'pnpm'
        if (-not $pnpmExe -and $npmExe) {
            try {
                & $npmExe install -g pnpm@11.21.0
                Refresh-Path
            }
            catch { Write-Host "  npm 全局安装失败。" -ForegroundColor Gray }
            $pnpmExe = Get-ToolPath 'pnpm'
        }
        if (-not $pnpmExe) {
            Write-Host "  [ERROR] 未能自动安装 pnpm。请手动执行：npm install -g pnpm@11.21.0" -ForegroundColor Red
            $toolFailed = $true
        }
    }

    if (-not $cargoExe) {
        Write-Step "安装 Rust 工具链（winget: Rustlang.Rustup）"
        if ($wingetExe) {
            winget install Rustlang.Rustup --accept-package-agreements --accept-source-agreements --silent
            Refresh-Path
            $cargoExe = Get-ToolPath 'cargo'
        }
        if (-not $cargoExe) {
            Write-Host "  [ERROR] 未能自动安装 Rust。请手动安装 rustup：https://rustup.rs/" -ForegroundColor Red
            $toolFailed = $true
        }
    }
}
else {
    $toolFailed = (-not $nodeExe) -or (-not $pnpmExe) -or (-not $cargoExe)
}

if ($toolFailed) {
    Write-Host ""
    Write-Host "  必要工具缺失，无法继续。请安装缺失工具后重跑本脚本。" -ForegroundColor Red
    Pop-Location
    exit 1
}

# rust-toolchain.toml 已固定 1.97.1 + clippy + rustfmt，首次 cargo 调用会自动安装/校准
if ($rustupExe) {
    & $rustupExe component add clippy rustfmt
}

# ---------------------------------------------------------------------------
# 3. 按顺序执行四条质量门禁（与 CI 一致）
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host " 注意：首次运行 pnpm check 需要完整编译 Rust 依赖（tauri 等），耗时较长属正常。" -ForegroundColor DarkGray

$failed = 0

# 1/4 依赖安装
$failed += Invoke-Gate "1/4  pnpm install --frozen-lockfile" { pnpm install --frozen-lockfile }

# 2/4 pnpm check（依赖安装失败则跳过）
if ((Get-LastCode) -eq 0) {
    $failed += Invoke-Gate "2/4  pnpm check" { pnpm check }
}
else {
    $script:Skipped.Add('pnpm check')
    Write-Host "  依赖安装失败，跳过 pnpm check。" -ForegroundColor Red
}

# 3/4 pnpm build（上一步失败则跳过）
if ((Get-LastCode) -eq 0) {
    $failed += Invoke-Gate "3/4  pnpm build" { pnpm build }
}
else {
    $script:Skipped.Add('pnpm build')
    Write-Host "  pnpm check 未通过，跳过 pnpm build。" -ForegroundColor Red
}

# 4/4 security:audit
if ($SkipAudit) {
    $script:Skipped.Add('pnpm security:audit')
}
elseif ((Get-LastCode) -eq 0) {
    # cargo-audit 缺失时先安装（与 CI 的 cargo install cargo-audit --locked 一致）
    if (-not (Get-ToolPath 'cargo-audit')) {
        Write-Step "安装 cargo-audit（首次约需数分钟）"
        cargo install cargo-audit --locked
        Refresh-Path
    }
    $failed += Invoke-Gate "4/4  pnpm security:audit" { pnpm security:audit }
}
else {
    $script:Skipped.Add('pnpm security:audit')
    Write-Host "  前面的步骤失败，跳过 pnpm security:audit。" -ForegroundColor Red
}

# ---------------------------------------------------------------------------
# 4. 汇总
# ---------------------------------------------------------------------------
Write-Step "结果汇总"
$allGates = @(
    'pnpm install --frozen-lockfile',
    'pnpm check',
    'pnpm build',
    'pnpm security:audit'
)
foreach ($name in $allGates) {
    $r = $script:GateResults | Where-Object { $_.Name -like ('*' + $name + '*') } | Select-Object -First 1
    if ($null -ne $r) {
        $mark = if ($r.Code -eq 0) { 'PASS' } else { 'FAIL' }
        $color = if ($r.Code -eq 0) { 'Green' } else { 'Red' }
        Write-Host ("  [{0}] {1}  (exit {2})" -f $mark, $r.Name, $r.Code) -ForegroundColor $color
    }
    elseif ($script:Skipped -contains $name) {
        Write-Host ("  [SKIP] {0}" -f $name) -ForegroundColor DarkGray
    }
    else {
        Write-Host ("  [--] {0}  (未执行)" -f $name) -ForegroundColor DarkGray
    }
}

if ($failed -eq 0) {
    Write-Host ""
    Write-Host "  ✔ 全部质量门禁通过。" -ForegroundColor Green
}
else {
    Write-Host ""
    Write-Host ("  ✘ 共 " + $failed + " 个步骤失败，请查看上方日志定位问题。" ) -ForegroundColor Red
}

Pop-Location
exit $failed
