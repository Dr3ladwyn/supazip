#!/usr/bin/env bash
# Local + CI entry point for SupaZip.
#
# Full content (fmt / build / test / clippy / doc / GUI) is added in
# a later commit. This stub is enough to make the directory exist and
# to be executable from day one.
set -euo pipefail
cd "$(dirname "$0")/../supazip"

echo "==> SupaZip CI (stub — full content lands in step 12)"
echo "    working dir: $(pwd)"
echo "    rustc:       $(rustc --version 2>/dev/null || echo 'not installed')"
echo "    cargo:       $(cargo --version 2>/dev/null || echo 'not installed')"
