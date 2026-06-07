# Changelog

All notable changes to SupaZip are documented in this file. Versions follow
[Semantic Versioning](https://semver.org/). The first published release is
`0.1.0`.

## [0.3.0] - 2026-06-07

### Added
- **GUI drag-and-drop** via egui builtin `ctx.input(|i| &i.raw.dropped_files)`.
- **Right-click context menu** on file list rows: Extract here, Extract to, Test entry, Copy path.
- **Modal progress dialog** with deterministic progress bar and Cancel button.
- **Virtualized file list** via `egui::ScrollArea::show_rows` (5000+ entries render smoothly).
- **Recent files** persisted as JSON in `dirs::data_local_dir()/supazip/recent_files.json`, capped at 10.
- **Password dialog** (modal, show/hide toggle, Enter to submit).
- **Native menu bar** (File / Edit / View / Help) via eframe `with_menu`; native on macOS, in-app elsewhere.
- Keyboard shortcuts: `Ctrl+O` / `Ctrl+E` / `Ctrl+W` / `Ctrl+Q`, `F1`.
- Property-based tests (`proptest`): 5 round-trip invariants covering zip, 7z, tar, tar.gz, tar.xz.
- Criterion benchmarks: 80 bench IDs across create / list / extract / test × 5 backends × 3 sizes.
- **GitHub Releases workflow** with minisign-signed checksums and CycloneDX SBOM (matrix: windows, ubuntu, macos × x86_64, aarch64).

### Changed
- `supazip-gui/Cargo.toml`: `dirs`, `serde`, `serde_json`, `chrono` promoted to runtime `[dependencies]`.
- `supazip-core/Cargo.toml`: `proptest` and `criterion` added to dev-deps.
- `proptest` revealed a safe_join over-strictness: rejects any `..` substring, not just `..` path components. Documented in decision log; fix scheduled for 0.4.

### Known limitations
- Local toolchain rustc 1.88 vs required 1.92 — `cargo check`/`test` skipped locally; CI is the authoritative gate.
- macOS release artifacts are not code-signed yet (added in 0.4).
- Wayland drag-and-drop is limited to status-bar hint (full support in 1.1).

## [0.2.0] - 2026-06-06

### Added
- TAR, TAR.GZ and TAR.XZ backends (pure-Rust stack: `tar`, `flate2`,
  `xz`).
- Brotli and Zstandard compression methods for ZIP create.
- `Limits::max_compression_ratio` (zip-bomb defence); enforced in 7z
  extract.
- cargo-fuzz harness skeleton (15 targets, 60s CI smoke).
- `assets/i18n/de.toml` stub (third locale), plural-section format.
- crates.io metadata: `keywords`, `categories`, `authors`, `description`
  on `supazip-core` and `supazip-cli`; `publish = false` on
  `supazip-gui`. `docs/publishing.md` documents the manual publish flow.

## [Unreleased]

### Added
- `Limits` (`max_archive_size`, `max_entry_count`, `max_entry_size`)
  threaded through `ArchiveFormat::list/extract/create/test`. The CLI
  honours `SUPAZIP_MAX_ARCHIVE_SIZE` (bytes with optional `K`/`M`/`G`
  suffix).
- Path-traversal guard `SevenZBackend::safe_join` for the 7z backend;
  `enclosed_name` covers the ZIP side. Both backends are tested.
- `ArchiverError::TooLarge(String)` for the limits failures.
- `ArchiverError::InvalidArchive { source, .. }` and
  `UnsupportedFormat { source, .. }` as struct variants so the original
  error from the underlying crate is preserved via `#[source]`. New
  `ArchiverError::invalid` and `invalid_with_source` helpers.
- ZIP create honours `--password` (AES-256, AE-2 vendor version).
- 7z create extracts correctly to a destination directory; safe-join and
  per-entry size limit; cancellation short-circuits to
  `ArchiverError::Cancelled` via the new `map_sevenz_error` arm.
- CLI atomic `create` through `tempfile::NamedTempFile` +
  `persist`. Drops the previous in-place write that could leave a
  half-written archive on Ctrl-C.
- CLI `--compression` flag (default `deflate`) for `create`.
- CLI env-filter tracing via `RUST_LOG` / `SUPAZIP_LOG`. Exit code 130
  on `ArchiverError::Cancelled`.
- `scripts/ci.sh` (POSIX) and `scripts/ci.ps1` (Windows PowerShell)
  local CI entry point. Mirrors the GitHub Actions job.
- `cargo fmt --check` and `cargo clippy -D warnings` are clean.
- Public-API rustdoc on `Limits`, `ArchiveFormat`, `ArchiveEntry`,
  `CreateOptions`, `ProgressState`, `ChannelProgress`, `NoOpProgress`,
  and `ArchiverError`.
- GUI skeleton: toolbar (Open / Extract / Create / Test), file list,
  status bar; all wired through `supazip_core` to keep behaviour
  identical to the CLI.

### Changed
- `ArchiveFormat::extract` now takes `dest_dir: &Path` instead of
  `Box<dyn WriteSeek>`. The CLI no longer chdir-s to emulate `--out` and
  the `NullDest` shim is gone.
- `ArchiverError` variants `InvalidArchive` and `UnsupportedFormat`
  are now struct variants; all call sites in `core` and `cli` updated.
- `tokio` got the `signal` feature so a future `tokio::signal::ctrl_c`
  hook can wire to the existing cancellation flag.
- `thiserror` upgraded to `2`.
- README updated to reflect the new API, security posture, and test
  coverage.

## [0.1.0] - 2026-05-25

### Added
- Initial engine: `supazip-core` with `ArchiveFormat` trait and
  `ZipBackend` / `SevenZBackend`.
- CLI: `list` / `extract` / `create` / `test` with `--password`,
  `--format`, `--entry`, `--all`.
- GUI: placeholder window.
- Unit + integration tests for the happy paths.
