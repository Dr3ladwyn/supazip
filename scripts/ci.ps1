# SupaZip local CI entry point for Windows PowerShell.
#
# Mirrors scripts/ci.sh so the same local workflow works on the Windows
# dev machines (the GitHub Actions runner is also Windows). Usage:
#
#   .\scripts\ci.ps1                # default: build + test
#   .\scripts\ci.ps1 -NoFmt         # skip rustfmt check
#   .\scripts\ci.ps1 -NoClippy      # skip clippy
#   .\scripts\ci.ps1 -Workspace     # also build the GUI crate
#   .\scripts\ci.ps1 -Release       # build in release mode

# SupaZip local CI entry point for Windows PowerShell.
#
# Mirrors scripts/ci.sh so the same local workflow works on the Windows
# dev machines (the GitHub Actions runner is also Windows). Usage:
#
#   .\scripts\ci.ps1                # default: build + test
#   .\scripts\ci.ps1 -NoFmt         # skip rustfmt check
#   .\scripts\ci.ps1 -NoClippy      # skip clippy
#   .\scripts\ci.ps1 -Workspace     # also build the GUI crate
#   .\scripts\ci.ps1 -Release       # build in release mode
#
# Using a top-level `param(...)` block exposed a parser quirk in older
# PowerShell where the absolute workspace path looked like a switch. The
# env-var based dispatch below is the workaround that does not trip it.

$ErrorActionPreference = "Stop"
# Workspace lives under supazip/; cd into it so `cargo build -p ...` works.
$root = Resolve-Path (Join-Path (Split-Path -Parent $PSCommandPath) "..")
$workspace = Join-Path $root.Path "supazip"
Set-Location $workspace

$NoFmt = $env:SUPPAZIP_CI_NO_FMT -eq "1"
$NoClippy = $env:SUPPAZIP_CI_NO_CLIPPY -eq "1"
$Workspace = $env:SUPAAZIP_CI_WORKSPACE -eq "1"
$Release = $env:SUPAAZIP_CI_RELEASE -eq "1"

$cargoFlags = @()
if ($Release) { $cargoFlags += "--release" }

Write-Host "==> cargo build (core+cli)"
& cargo build @cargoFlags -p supazip-core -p supazip-cli
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

if ($Workspace) {
    Write-Host "==> cargo build (gui)"
    & cargo build @cargoFlags -p supazip-gui
    if ($LASTEXITCODE -ne 0) { throw "cargo build gui failed" }
}

Write-Host "==> cargo test (core+cli)"
& cargo test @cargoFlags -p supazip-core -p supazip-cli
if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }

if ($Workspace) {
    Write-Host "==> cargo test (gui)"
    & cargo test @cargoFlags -p supazip-gui
    if ($LASTEXITCODE -ne 0) { throw "cargo test gui failed" }
}

if (-not $NoFmt) {
    Write-Host "==> cargo fmt --check"
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed" }
}

if ($env:SUPAAZIP_CI_NO_DESIGN -ne "1") {
    $designScript = Join-Path (Split-Path -Parent $PSCommandPath) "ci-design.ps1"
    if (Test-Path $designScript) {
        & $designScript
    } else {
        Write-Host "==> design checks: skipped (scripts/ci-design.ps1 not found)"
    }
}

if (-not $NoClippy) {
    Write-Host "==> cargo clippy"
    & cargo clippy @cargoFlags -p supazip-core -p supazip-cli --no-deps -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed" }
    if ($Workspace) {
        & cargo clippy @cargoFlags -p supazip-gui --no-deps -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw "cargo clippy gui failed" }
    }
}

Write-Host "==> all checks passed"
