#!/usr/bin/env bash
# SupaZip local CI entry point.
#
# This script runs the same checks we expect a CI server to run. It is the
# "single source of truth" for whether a change is good to land: if
# `scripts/ci.sh` is green locally, the corresponding GitHub Actions job
# (see `.github/workflows/ci.yml`) should also be green.
#
# Usage:
#   scripts/ci.sh                # default: build + test (core+cli)
#   scripts/ci.sh --no-fmt       # skip rustfmt check
#   scripts/ci.sh --no-clippy    # skip clippy
#   scripts/ci.sh --workspace    # also build the GUI crate
#   scripts/ci.sh --release      # build in release mode
#
# Environment variables:
#   CI_BUILD_GUI=1                # also build the GUI crate (supazip-gui),
#                                 # even on Linux. The GitHub Actions `gui`
#                                 # job sets this implicitly by being a
#                                 # windows-latest runner; the `core-cli`
#                                 # job leaves it unset so the GUI crate
#                                 # is skipped on ubuntu-latest.
#                                 # GUI tests are skipped locally on non-Windows
#                                 # hosts unless CI_BUILD_GUI=1 is set.

set -euo pipefail

# Workspace lives under supazip/; cd into it so `cargo build -p ...` works.
cd "$(dirname "$0")/../supazip"

no_fmt=0
no_clippy=0
no_doc=0
workspace=0
release=0

for arg in "$@"; do
    case "$arg" in
        --no-fmt)     no_fmt=1 ;;
        --no-clippy)  no_clippy=1 ;;
        --no-doc)     no_doc=1 ;;
        --workspace)  workspace=1 ;;
        --release)    release=1 ;;
        -h|--help)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *)
            echo "unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

# Treat CI_BUILD_GUI=1 as an additional opt-in for the GUI build.
build_gui=0
if [ "$workspace" -eq 1 ] || [ "${CI_BUILD_GUI:-}" = "1" ]; then
    build_gui=1
fi

# Fuzz smoke is intentionally not part of the local script. The corresponding
# `fuzz-smoke` job in `.github/workflows/ci.yml` runs `cargo +nightly fuzz
# run` for 60 s on the GitHub Actions runner; locally we do not assume a
# nightly toolchain is installed. See `supazip-core/fuzz/README.md` for the
# manual runbook.
if [ -n "${RUN_FUZZ_SMOKE:-}" ]; then
    echo "==> fuzz smoke (RUN_FUZZ_SMOKE set)"
    (cd supazip-core && cargo +nightly fuzz run zip_list -- -max_total_time=60)
    (cd supazip-core && cargo +nightly fuzz run sevenz_extract -- -max_total_time=60)
fi

CARGO_FLAGS=()
if [ "$release" -eq 1 ]; then
    CARGO_FLAGS+=(--release)
fi

echo "==> cargo build (core+cli)"
cargo build "${CARGO_FLAGS[@]}" -p supazip-core -p supazip-cli

if [ "$build_gui" -eq 1 ]; then
    echo "==> cargo build (gui)"
    cargo build "${CARGO_FLAGS[@]}" -p supazip-gui
else
    echo "[skip] gui build (set --workspace or CI_BUILD_GUI=1 to enable)"
fi

echo "==> cargo test (core+cli)"
cargo test "${CARGO_FLAGS[@]}" -p supazip-core -p supazip-cli

if [ "$build_gui" -eq 1 ]; then
    echo "==> cargo test (gui)"
    cargo test "${CARGO_FLAGS[@]}" -p supazip-gui
fi

if [ "$no_fmt" -eq 0 ]; then
    echo "==> cargo fmt --check"
    cargo fmt --all -- --check
fi

if [ "$no_doc" -eq 0 ]; then
    echo "==> cargo doc --no-deps"
    cargo doc "${CARGO_FLAGS[@]}" -p supazip-core -p supazip-cli --no-deps
fi

if [ "$no_clippy" -eq 0 ]; then
    echo "==> cargo clippy"
    cargo clippy "${CARGO_FLAGS[@]}" -p supazip-core -p supazip-cli --no-deps -- -D warnings
    if [ "$build_gui" -eq 1 ]; then
        cargo clippy "${CARGO_FLAGS[@]}" -p supazip-gui --no-deps -- -D warnings
    fi
fi

echo "==> all checks passed"
