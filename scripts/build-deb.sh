#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../supazip"
cargo install cargo-deb 2>/dev/null || true
cargo deb -p supazip-cli --locked
