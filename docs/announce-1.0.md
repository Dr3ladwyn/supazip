# SupaZip 1.0: A cross-platform archive manager in Rust

> SupaZip 1.0 is the first stable release of a cross-platform archive
> manager written in Rust. It supports ZIP, 7z, TAR, TAR.GZ, and TAR.XZ
> backends with AES-256 encryption, resource limits, path-traversal
> protection, and atomic writes.

---

## What is SupaZip?

SupaZip is an open-source archive manager that runs on Windows, Linux,
and macOS. It provides three front-ends -- a desktop GUI, a CLI, and a
Rust library -- all powered by a single archive engine
(`supazip-core`). The same extraction, creation, and testing logic runs
regardless of which front-end you use.

SupaZip is built entirely in Rust with no C dependencies on Windows.
The binary is self-contained: no runtime DLLs, no system libraries, no
interpreter.

**Repository:** https://github.com/your-org/supazip
**License:** MIT OR Apache-2.0

## Features

### Five archive backends

| Format      | Read | Write | Encryption        |
|-------------|------|-------|-------------------|
| ZIP         | Yes  | Yes   | AES-256 (AE-2)    |
| 7z          | Yes  | Yes   | AES-256 + LZMA2   |
| TAR         | Yes  | Yes   | N/A               |
| TAR.GZ      | Yes  | Yes   | N/A               |
| TAR.XZ      | Yes  | Yes   | N/A               |

All backends share a common `ArchiveFormat` trait. Adding a new format
is a matter of implementing the trait and registering the extension.

### CLI

- `list`, `extract`, `create`, `test` subcommands.
- `--password` for encrypted archives.
- `--output json|yaml|text` for machine-readable output.
- Shell completions for bash, zsh, fish, PowerShell, and Elvish.
- Man page generation (`supazip man --out-dir <DIR>`).
- `Ctrl-C` cancellation with exit code 130.
- Environment-based logging: `RUST_LOG=debug supazip ...`.

### Desktop GUI

- Drag-and-drop archive opening.
- Right-click context menu (Extract here, Extract to, Test entry, Copy path).
- Progress dialog with cancel button and virtualised entry list.
- Recent files (persisted across sessions).
- Password dialog with show/hide toggle.
- Settings window (language, max archive size, debug overlay).
- Native menu bar (File / Edit / View / Help) with keyboard shortcuts.

### Internationalisation

- English (en), Russian (ru), German (de).
- Hand-rolled plural runtime with CLDR rules.
- All user-facing strings externalised to TOML files.

### Accessibility

- WCAG 2.1 AA audited (17 PASS, 2 PARTIAL).
- Keyboard navigation throughout the GUI.
- Screen reader smoke test recipes for NVDA, VoiceOver, and Orca.

### Security

- **Resource limits**: `max_archive_size`, `max_entry_count`,
  `max_entry_size`, `max_compression_ratio`. Guards against
  zip-bombs and decompression bombs.
- **Path-traversal protection**: zip-slip / 7z-slip attacks are
  refused before any bytes hit the filesystem.
- **Atomic writes**: archives are written through a temp file and
  atomically renamed. Partial writes never corrupt existing files.
- **Cancellation safety**: Ctrl-C cleans up temp files and exits
  cleanly.
- **Encryption**: AES-256 for both ZIP and 7z.
- **Disclosure policy**: See [SECURITY.md](../SECURITY.md).

## Architecture

SupaZip is a Cargo workspace with three crates:

```
supazip-core   <-- Archive engine (no GUI deps)
    ^
    |
    +--- supazip-cli   (CLI front-end)
    +--- supazip-gui   (eframe/egui desktop app)
```

The dependency graph is one-way: `core` <- `cli`, `core` <- `gui`.
The GUI and CLI never import each other.

### Tech stack

| Component    | Crate           | Version       |
|--------------|-----------------|---------------|
| 7z backend   | `sevenz-rust`   | 0.6.1 (AES)  |
| ZIP backend  | `zip`           | 2.4.2         |
| TAR stack    | `tar`+`flate2`+`lzma-rs` | Pure Rust |
| GUI          | `egui`/`eframe` | 0.34.1        |
| CLI          | `clap`          | 4.6 (derive)  |
| Async        | `tokio`         | 1.50          |
| Errors       | `thiserror`     | 2             |

All dependencies are MIT, Apache-2.0, or compatible open-source
licenses. No copyleft or proprietary dependencies.

## Performance

<!-- Fill from criterion benchmark runs. Example format: -->

Benchmark results are measured with Criterion on a dedicated CI runner.
Baseline stored under `supazip-core/benches/baselines/`.

| Operation | Format | 100 files (10 MB) | 1000 files (100 MB) | 10,000 files (1 GB) |
|-----------|--------|-------------------|---------------------|---------------------|
| Create    | ZIP    | _TBD_             | _TBD_               | _TBD_               |
| Create    | 7z     | _TBD_             | _TBD_               | _TBD_               |
| Create    | TAR.GZ | _TBD_            | _TBD_               | _TBD_               |
| Extract   | ZIP    | _TBD_             | _TBD_               | _TBD_               |
| Extract   | 7z     | _TBD_             | _TBD_               | _TBD_               |
| Extract   | TAR.GZ | _TBD_            | _TBD_               | _TBD_               |
| List      | ZIP    | _TBD_             | _TBD_               | _TBD_               |
| List      | 7z     | _TBD_             | _TBD_               | _TBD_               |
| Test      | ZIP    | _TBD_             | _TBD_               | _TBD_               |

Run your own benchmarks:

```bash
cargo bench -p supazip-core --bench engine
```

## Quality

- **Test coverage:** 95%+ of `supazip-core` (measured by `cargo-llvm-cov`).
- **Unit tests:** 100+ tests covering list, extract, test, create,
  encryption, error variants, limits, cancellation, and path-traversal.
- **Property-based tests:** proptest round-trips for all 5 backends.
- **Fuzzing:** cargo-fuzz harnesses for list, extract, and create
  across all backends.
- **Static analysis:** `cargo clippy -D warnings`, `cargo-deny`
  (advisories, bans, licences, sources), `cargo-audit`.
- **CI matrix:** Windows, Ubuntu, macOS (x86_64), macOS (arm64).

## Installation

### Homebrew (macOS, Linux)

```bash
brew tap your-org/supazip
brew install supazip
```

### AUR (Arch Linux)

```bash
yay -S supazip
# or
paru -S supazip
```

### winget (Windows)

```powershell
winget install SupaZip.SupaZip
```

### scoop (Windows)

```powershell
scoop bucket add supazip https://github.com/your-org/scoop-supazip
scoop install supazip
```

### Nix

```bash
nix profile install github:your-org/supazip#supazip-cli
```

### Docker

```bash
docker run --rm ghcr.io/your-org/supazip:latest --version
docker run --rm -v $(pwd):/data ghcr.io/your-org/supazip list /data/archive.zip
```

### crates.io

```bash
cargo install supazip-cli
```

### From source

```bash
git clone https://github.com/your-org/supazip.git
cd supazip/supazip
cargo build --release -p supazip-cli -p supazip-gui
```

Requires Rust >= 1.92. MSRV is tested in CI.

## Supply chain security

Every release includes:

- **CycloneDX SBOM** covering all first-party and transitive dependencies.
- **SHA-256 checksums** signed with [minisign](https://jedisct1.github.io/minisign/).
- **Reproducible-build verification** (the CI rebuilds from source and compares hashes).

Verify a release:

```bash
minisign -Vm supazip-linux-x64.tar.gz -P <public-key> -x supazip-linux-x64.tar.gz.minisig
sha256sum -c SHA256SUMS.txt
```

## What's next (1.1 plans)

- **Light theme.** The 1.0 GUI is dark-only. A light theme will be
  added based on user feedback.
- **Self-update.** `supazip update` subcommand for CLI self-update
  (deferred from 1.0 to reduce attack surface).
- **RAR support.** Read-only RAR support via `unar-bindings` (the
  format is proprietary and was out of scope for 1.0).
- **Async backends.** Optional async trait for backends, enabling
  network-based archive sources.
- **Plugin system.** Extensibility for custom archive formats and
  processing pipelines (post-1.0, if demand surfaces).

## Contributors

<!-- Add contributors here. Format: Name (@handle) - contributions -->
_See the full list of contributors on
[GitHub](https://github.com/your-org/supazip/graphs/contributors)._

## Links

- **Repository:** https://github.com/your-org/supazip
- **Documentation:** https://supazip.github.io/supazip/
- **crates.io:** https://crates.io/crates/supazip-cli
- **Rust forum announcement:** _TBD (post link after forum submission)_
- **Security policy:** [SECURITY.md](../SECURITY.md)
- **Changelog:** [CHANGELOG.md](../CHANGELOG.md)
- **Roadmap:** [ROADMAP.md](../ROADMAP.md)
