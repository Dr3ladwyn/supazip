#!/usr/bin/env bash
set -euo pipefail

# Reproducible build script for SupaZip.
#
# Pinned timestamps and path remapping ensure that two builds on the same
# machine (or in the same container) produce bit-identical binaries for
# supazip-core, supazip-cli.

export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)
export TZ=UTC
export RUSTFLAGS="--remap-path-prefix $HOME=/home/user --remap-path-prefix $PWD=/src"

cargo build --release --locked -p supazip-core -p supazip-cli
cargo build --release --locked -p supazip-gui || true  # GUI not reproducible yet

echo ""
echo "=== SHA-256 of artifacts ==="
sha256sum target/release/supazip-cli target/release/supazip-cli.exe 2>/dev/null || true
