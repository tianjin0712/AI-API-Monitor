param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectRoot,
    [switch]$SecurityAudit,
    [switch]$TauriBuild
)

$ErrorActionPreference = 'Stop'
$resolvedRoot = (Resolve-Path -LiteralPath $ProjectRoot).Path
$requiredFiles = @('package.json', 'src-tauri/Cargo.toml', '.github/workflows/quality.yml')

foreach ($relativePath in $requiredFiles) {
    if (-not (Test-Path -LiteralPath (Join-Path $resolvedRoot $relativePath))) {
        throw "Not an AI API Monitor repository: missing $relativePath"
    }
}

function Invoke-GateStep {
    param([string]$Name, [scriptblock]$Action)
    Write-Host "`n[$Name]"
    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
    Write-Host "[$Name] PASS"
}

Push-Location $resolvedRoot
try {
    if ($SecurityAudit -and -not (Get-Command cargo-audit -ErrorAction SilentlyContinue)) {
        throw "Security audit requested, but cargo-audit is not installed. Install it explicitly with 'cargo install cargo-audit --locked', then rerun."
    }

    Invoke-GateStep 'Node version' { node --version }
    Invoke-GateStep 'pnpm version' { pnpm --version }
    Invoke-GateStep 'Rust version' { rustc --version }
    Invoke-GateStep 'Cargo version' { cargo --version }
    Invoke-GateStep 'Git status' { git status --short }
    Invoke-GateStep 'Git whitespace' { git diff --check }
    Invoke-GateStep 'Tracked secret files' {
        $secretFiles = @(git ls-files | Select-String -Pattern '\.(key|p12|pfx|pem)$')
        if ($secretFiles.Count -gt 0) {
            $secretFiles | ForEach-Object { Write-Host $_.Line }
            exit 2
        }
    }
    Invoke-GateStep 'Project checks' { pnpm check }
    Invoke-GateStep 'Frontend build' { pnpm build }

    if ($SecurityAudit) {
        Invoke-GateStep 'Dependency security audit' { pnpm security:audit }
    }

    if ($TauriBuild) {
        Invoke-GateStep 'Tauri build' { pnpm tauri build }
    }

    Write-Host "`nQUALITY GATE PASS"
}
finally {
    Pop-Location
}
