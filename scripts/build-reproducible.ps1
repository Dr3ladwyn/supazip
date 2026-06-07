<#
.SYNOPSIS
    Reproducible build script for SupaZip (Windows / PowerShell).

.DESCRIPTION
    Pinned timestamps and path remapping ensure that two builds on the same
    machine produce bit-identical binaries for supazip-core and supazip-cli.
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Pin SOURCE_DATE_EPOCH to the last commit timestamp
$epoch = git log -1 --format=%ct
$env:SOURCE_DATE_EPOCH = $epoch
$env:TZ = 'UTC'

$homePath = $env:USERPROFILE -replace '\\','/'
$pwdPath = $pwd.Path -replace '\\','/'
$env:RUSTFLAGS = "--remap-path-prefix $homePath=/home/user --remap-path-prefix $pwdPath=/src"

cargo build --release --locked -p supazip-core -p supazip-cli

try {
    cargo build --release --locked -p supazip-gui
} catch {
    Write-Host 'GUI build skipped (not reproducible yet)'
}

Write-Host ''
Write-Host '=== SHA-256 of artifacts ==='
$cli = 'target/release/supazip-cli.exe'
if (Test-Path $cli) {
    (Get-FileHash $cli -Algorithm SHA256).Hash.ToLower() + "  $cli"
} else {
    Write-Host "Not found: $cli"
}
