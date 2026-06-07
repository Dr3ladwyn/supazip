# Reproducible Builds

Two builds of the same source tree on the same machine should produce
bit-identical binaries.  The helper scripts
`scripts/build-reproducible.sh` (POSIX) and `scripts/build-reproducible.ps1`
(Windows) set the required environment variables.

## How it works

### `SOURCE_DATE_EPOCH`

Set to the Unix timestamp of the latest git commit.  Rust and many C
dependencies embed build timestamps; `SOURCE_DATE_EPOCH` replaces them with
a deterministic value.  The `TZ=UTC` export removes any local-timezone
variation.

### `--remap-path-prefix`

```
RUSTFLAGS="--remap-path-prefix $HOME=/home/user --remap-path-prefix $PWD=/src"
```

Compiler diagnostics, `file!()` macros, and panic messages normally contain
the absolute path of the source tree.  Remapping replaces `$HOME` and `$PWD`
with fixed placeholders so two checkouts in different directories produce the
same binary.

## Verification

```bash
# Build once
scripts/build-reproducible.sh

# Note the SHA-256
sha256sum target/release/supazip-cli

# Build again (or on another machine with the same toolchain)
scripts/build-reproducible.sh

# Compare — the hashes must match
sha256sum target/release/supazip-cli
```

On Windows, use `scripts/build-reproducible.ps1` and compare with
`Get-FileHash`.

## Limitations

- **GUI (`supazip-gui`)**: eframe/egui may embed timestamps in native
  resources (icons, manifests).  The GUI build is attempted but failures are
  non-fatal; reproducibility is currently guaranteed only for **CLI** and
  **core** artifacts.
- **Cross-machine**: different Rust toolchain patch versions or system
  libraries can change codegen.  Use the exact toolchain version from
  `rust-toolchain.toml` (if present) or pin via `rustup`.
- **Lockfile required**: `--locked` ensures identical dependency resolution.
