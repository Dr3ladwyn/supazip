# SupaZip local CI: design-system sync checks.
#
# Runs the two stdlib-only Python design checks (token sync and i18n sync)
# that live under `design/scripts/`. Mirrors the style of `scripts/ci.ps1` so
# it can be invoked from there or standalone:
#
#   .\scripts\ci-design.ps1            # run both checks
#   .\scripts\ci-design.ps1 -NoPython  # skip with a warning (envs without Python)
#
# Opt-out env vars:
#   SUPAAZIP_CI_NO_DESIGN=1   also skips the step entirely (exits 0)

param(
    [switch]$NoPython
)

$ErrorActionPreference = "Stop"

# Workspace lives one level up from this script (scripts/ is at the repo root).
$root = Resolve-Path (Join-Path (Split-Path -Parent $PSCommandPath) "..")
Set-Location $root.Path

if ($env:SUPAAZIP_CI_NO_DESIGN -eq "1") {
    Write-Host "==> design checks: skipped (SUPAAZIP_CI_NO_DESIGN=1)"
    exit 0
}

if ($NoPython) {
    Write-Warning "==> design checks: skipped (NoPython mode; Python not required)"
    exit 0
}

# Pick a Python interpreter. Try `python` first, then the Windows launcher
# `py -3`. Fail with a clear message if neither is available; CI always has
# Python via actions/setup-python, but local devs may not.
$pythonCmd = $null
try {
    $pythonCmd = (Get-Command python -ErrorAction Stop).Source
} catch {
    try {
        $pyLauncher = (Get-Command py -ErrorAction Stop).Source
        $pythonCmd = "$pyLauncher -3"
    } catch {
        Write-Error "==> design checks: FAILED (no Python interpreter found on PATH; install Python 3.11+ or set -NoPython)"
        exit 1
    }
}

Write-Host "==> design checks: using $pythonCmd"

$checks = @(
    "design/scripts/check_tokens.py"
    "design/scripts/check_i18n.py"
)

foreach ($script in $checks) {
    $fullPath = Join-Path $root.Path $script
    if (-not (Test-Path $fullPath)) {
        Write-Error "==> design checks: FAILED (missing $script)"
        exit 1
    }
    Write-Host "==> python $script"
    & $pythonCmd $script
    if ($LASTEXITCODE -ne 0) { throw "$script failed" }
}

Write-Host "==> design checks: passed"
