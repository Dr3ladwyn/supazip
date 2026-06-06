#!/usr/bin/env bash
# SupaZip local CI entry point.
#
# This script runs the same checks we expect a CI server to run. It is the
# "single source of truth" for whether a change is good to land: if
# `scripts/ci.sh` is green locally, the corresponding GitHub Actions job
# (see `.github/workflows/ci.yml`) should also be green.
#
# Usage:
#   scripts/ci.sh                # default: build + test
#   scripts/ci.sh --no-fmt       # skip rustfmt check
#   scripts/ci.sh --no-clippy    # skip clippy
#   scripts/ci.sh --workspace    # also build the GUI crate
#   scripts/ci.sh --release      # build in release mode

set -euo pipefail

# Workspace lives under supazip/; cd into it so `cargo build -p ...` works.
cd "$(dirname "$0")/../supazip"

no_fmt=0
no_clippy=0
workspace=0
release=0

for arg in "$@"; do
    case "$arg" in
        --no-fmt)     no_fmt=1 ;;
        --no-clippy)  no_clippy=1 ;;
        --workspace)  workspace=1 ;;
        --release)    release=1 ;;
        -h|--help)
            sed -n '2,15p' "$0"
            exit 0
            ;;
        *)
            echo "unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

CARGO_FLAGS=()
if [ "$release" -eq 1 ]; then
    CARGO_FLAGS+=(--release)
fi

echo "==> cargo build (core+cli)"
cargo build "${CARGO_FLAGS[@]}" -p supazip-core -p supazip-cli

if [ "$workspace" -eq 1 ]; then
    echo "==> cargo build (gui)"
    cargo build "${CARGO_FLAGS[@]}" -p supazip-gui
fi

echo "==> cargo test (core+cli)"
cargo test "${CARGO_FLAGS[@]}" -p supazip-core -p supazip-cli

if [ "$workspace" -eq 1 ]; then
    echo "==> cargo test (gui)"
    cargo test "${CARGO_FLAGS[@]}" -p supazip-gui
fi

if [ "$no_fmt" -eq 0 ]; then
    echo "==> cargo fmt --check"
    cargo fmt --all -- --check
fi

if [ "$no_clippy" -eq 0 ]; then
    echo "==> cargo clippy"
    cargo clippy "${CARGO_FLAGS[@]}" -p supazip-core -p supazip-cli --no-deps -- -D warnings
    if [ "$workspace" -eq 1 ]; then
        cargo clippy "${CARGO_FLAGS[@]}" -p supazip-gui --no-deps -- -D warnings
    fi
fi

echo "==> all checks passed"
