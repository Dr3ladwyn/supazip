# SupaZip local CI entry point for Windows PowerShell.
#
# Mirrors scripts/ci.sh so the same local workflow works on the Windows
# dev machines (the GitHub Actions runner is also Windows). Usage:
#
#   .\scripts\ci.ps1                # default: build + test
#   .\scripts\ci.ps1 -NoFmt         # skip rustfmt check
#   .\scripts\ci.ps1 -NoClippy      # skip clippy
#   .\scripts\ci.ps1 -NoDoc         # skip cargo doc check
#   .\scripts\ci.ps1 -Workspace     # also build the GUI crate
#   .\scripts\ci.ps1 -Release       # build in release mode
#
# Environment variables:
#   $env:CI_BUILD_GUI = "1"          # also build the GUI crate.
#                                    # Mirrors the `gui` job in
#                                    # .github/workflows/ci.yml. The
#                                    # `core-cli` job on ubuntu-latest
#                                    # leaves it unset so the GUI build
#                                    # is skipped there.
#   $env:SUPAAZIP_CI_NO_FMT   = "1"  # skip rustfmt check
#   $env:SUPAAZIP_CI_NO_CLIPPY = "1" # skip clippy
#   $env:SUPAAZIP_CI_NO_DOC   = "1"  # skip cargo doc check
#   $env:SUPAAZIP_CI_WORKSPACE = "1" # also build the GUI crate
#   $env:SUPAAZIP_CI_RELEASE  = "1"  # build in release mode
#   $env:RUN_FUZZ_SMOKE       = "1"  # run a 60s cargo-fuzz smoke locally
#                                    # (requires a nightly toolchain)
#
# Using a top-level `param(...)` block exposed a parser quirk in older
# PowerShell where the absolute workspace path looked like a switch. The
# env-var based dispatch below is the workaround that does not trip it.

$ErrorActionPreference = "Stop"
# Workspace lives under supazip/; cd into it so `cargo build -p ...` works.
$root = Resolve-Path (Join-Path (Split-Path -Parent $PSCommandPath) "..")
$workspace = Join-Path $root.Path "supazip"
Set-Location $workspace

$NoFmt = $env:SUPAAZIP_CI_NO_FMT -eq "1"
$NoClippy = $env:SUPAAZIP_CI_NO_CLIPPY -eq "1"
$NoDoc = $env:SUPAAZIP_CI_NO_DOC -eq "1"
$Workspace = $env:SUPAAZIP_CI_WORKSPACE -eq "1" -or $env:CI_BUILD_GUI -eq "1"
$Release = $env:SUPAAZIP_CI_RELEASE -eq "1"

$cargoFlags = @()
if ($Release) { $cargoFlags += "--release" }

Write-Host "==> cargo build (core+cli)"
& cargo build @cargoFlags -p supazip-core -p supazip-cli --locked
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

if ($Workspace) {
    Write-Host "==> cargo build (gui)"
    & cargo build @cargoFlags -p supazip-gui --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo build gui failed" }
}

Write-Host "==> cargo test (core+cli)"
& cargo test @cargoFlags -p supazip-core -p supazip-cli --locked
if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }

if ($Workspace) {
    Write-Host "==> cargo test (gui)"
    & cargo test @cargoFlags -p supazip-gui --locked
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

if (-not $NoDoc) {
    Write-Host "==> cargo doc --no-deps"
    & cargo doc @cargoFlags -p supazip-core -p supazip-cli --no-deps --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo doc failed" }
}

if (-not $NoClippy) {
    Write-Host "==> cargo clippy"
    & cargo clippy @cargoFlags -p supazip-core -p supazip-cli --no-deps --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed" }
    if ($Workspace) {
        & cargo clippy @cargoFlags -p supazip-gui --no-deps --locked -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw "cargo clippy gui failed" }
    }
}

if ($env:RUN_FUZZ_SMOKE -eq "1") {
    Write-Host "==> fuzz smoke (RUN_FUZZ_SMOKE set)"
    Push-Location (Join-Path $workspace "supazip-core")
    try {
        & cargo +nightly fuzz run zip_list -- -max_total_time=60
        if ($LASTEXITCODE -ne 0) { throw "fuzz zip_list failed" }
        & cargo +nightly fuzz run sevenz_extract -- -max_total_time=60
        if ($LASTEXITCODE -ne 0) { throw "fuzz sevenz_extract failed" }
    } finally {
        Pop-Location
    }
}

Write-Host "==> all checks passed"
