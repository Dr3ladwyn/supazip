# Introduction

**SupaZip** is a production-grade, cross-platform archive manager written in Rust. It supports 7z, ZIP, TAR, TAR.GZ, and TAR.XZ formats through both a command-line interface and a graphical desktop application.

## Why SupaZip?

Existing archive tools either lack modern format support, ship with opaque C dependencies, or provide no library API for programmatic use. SupaZip addresses all three:

- **Pure Rust.** Every backend dependency (`sevenz-rust`, `zip`, `tar`, `flate2`, `lzma-rs`) is MIT/Apache-2.0 licensed with zero C bindings on Windows. The binary is self-contained.
- **Three front-ends, one engine.** The `supazip-core` crate is the shared engine. The CLI (`supazip-cli`) and GUI (`supazip-gui`) are thin wrappers that call into it. Bug fixes in the core benefit both interfaces.
- **Security-first.** Path-traversal protection, zip-bomb detection via compression-ratio limits, AES-256 encryption for ZIP and 7z, and configurable resource limits are baked in, not bolted on.

## Quick start

### Install from source

```bash
git clone https://github.com/your-org/supazip.git
cd supazip/supazip
cargo build --release -p supazip-cli
```

The binary lands at `target/release/supazip`.

### List an archive

```bash
supazip list archive.zip
```

### Extract

```bash
supazip extract archive.7z --out ./output
```

### Create

```bash
supazip create backup.tar.gz file1.txt file2.txt
```

### Test integrity

```bash
supazip test archive.zip
```

## Project layout

The workspace contains three crates:

| Crate | Path | Purpose |
|-------|------|---------|
| `supazip-core` | `supazip/supazip-core/` | Archive engine — formats, traits, errors, limits |
| `supazip-cli` | `supazip/supazip-cli/` | CLI front-end (`list`, `extract`, `create`, `test`, `completions`) |
| `supazip-gui` | `supazip/supazip-gui/` | Desktop GUI (eframe/egui + rfd) |

The design system is documented in [`DESIGN.md`](../../../DESIGN.md) at the repository root. Architecture decisions are tracked in [`memory-bank/decisionLog.md`](../../../memory-bank/decisionLog.md).

## Supported formats

| Format | Read | Write | Encryption |
|--------|------|-------|------------|
| ZIP | Yes | Yes | AES-256 (AE-2) |
| 7z | Yes | Yes | AES-256 + LZMA2 |
| TAR | Yes | Yes | No |
| TAR.GZ | Yes | Yes | No |
| TAR.XZ | Yes | Yes | No |

## License

SupaZip is dual-licensed under **MIT** and **Apache-2.0**.
