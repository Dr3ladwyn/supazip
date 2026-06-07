#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../supazip"
cargo install cargo-rpm 2>/dev/null || true
cargo rpm build -p supazip-cli --locked
