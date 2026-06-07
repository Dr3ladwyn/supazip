# Building

## Prerequisites

- **Rust 1.92+** (MSRV). Install via [rustup](https://rustup.rs/).
- **Platform build tools:**
  - Windows: Visual Studio Build Tools (C++ workload)
  - Linux: `build-essential`
  - macOS: Xcode Command Line Tools (`xcode-select --install`)

## Workspace layout

The workspace root is `supazip/Cargo.toml`:

```toml
[workspace]
members = ["supazip-core", "supazip-gui", "supazip-cli"]
resolver = "2"
```

## Build commands

### Debug build (all crates)

```bash
cd supazip
cargo build
```

### Release build

```bash
cargo build --release
```

### Build a single crate

```bash
# CLI only
cargo build --release -p supazip-cli

# GUI only
cargo build --release -p supazip-gui

# Core library only
cargo build -p supazip-core
```

## rust-toolchain.toml

The repository pins the Rust toolchain via `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
```

This ensures all contributors and CI use the same compiler. To override locally:

```bash
rustup override set 1.92
```

## Features

### Core

No optional features. All backends are always compiled.

### CLI

| Feature | Default | Description |
|---------|---------|-------------|
| `json-output` | Yes | `serde_json` for `--output json` |
| `yaml-output` | Yes | `serde_yaml` for `--output yaml` |

### GUI

| Feature | Default | Description |
|---------|---------|-------------|
| (none) | — | All GUI features are always enabled |

## Dependencies

### Runtime

| Crate | Version | Purpose |
|-------|---------|---------|
| `sevenz-rust` | 0.6 | 7z backend (AES-256 feature enabled) |
| `zip` | 2.4 | ZIP backend (chrono + zstd features) |
| `tar` | 0.4 | TAR reader/writer |
| `flate2` | 1 | Gzip compression (miniz_oxide backend) |
| `lzma-rs` | 0.3 | XZ / LZMA2 compression (pure Rust) |
| `thiserror` | 2 | Error derive macro |
| `serde` | 1 | Serialization (JSON, YAML output) |
| `chrono` | 0.4 | Date/time handling for archive entries |
| `clap` | 4 | CLI argument parsing |
| `clap_complete` | 4 | Shell completion generation |
| `eframe` / `egui` | 0.34 | GUI framework |
| `rfd` | 0.17 | Native file dialogs |
| `tokio` | 1 | Async runtime (GUI signal handling) |
| `tracing` | 0.1 | Structured logging (CLI) |
| `tempfile` | 3 | Atomic file creation |

### Dev-only

| Crate | Version | Purpose |
|-------|---------|---------|
| `criterion` | 0.5 | Benchmarks |
| `proptest` | 1 | Property-based tests |

## Output paths

| Artifact | Debug path | Release path |
|----------|-----------|--------------|
| CLI binary | `target/debug/supazip` | `target/release/supazip` |
| GUI binary | `target/debug/supazip-gui` | `target/release/supazip-gui` |
| Core library | `target/debug/libsupazip_core.rlib` | `target/release/libsupazip_core.rlib` |

## Reproducible builds

Reproducible builds are planned for milestone 0.4.0 via pinned `SOURCE_DATE_EPOCH` and sorted zip metadata. The script `scripts/build-reproducible.sh` will produce bit-for-bit identical archives across machines.
