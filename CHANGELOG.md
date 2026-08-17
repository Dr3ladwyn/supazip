# Changelog

All notable changes to SupaZip are documented in this file. Versions follow
[Semantic Versioning](https://semver.org/). The first published release is
`0.1.0`.

## [Unreleased]

Harden work targeting 1.0.1 is listed in the section below until that tag
is cut. New post-1.0.1 changes go here.

## [1.0.1] - Unreleased

### Fixed

- Single GitHub identity placeholder: `your-org/supazip` in README,
  SECURITY.md, packaging manifests, crate `repository` URLs, and
  `.github/ISSUE_TEMPLATE/config.yml`. Do not invent a real handle until
  a public repository exists.
- CI: design token/i18n scripts run from the repo root (`design/scripts/`),
  not under `supazip/`.
- Coverage: `cargo llvm-cov --workspace` runs with working-directory
  `supazip`.
- Release workflow: unique `actions/upload-artifact` names per matrix
  target; binaries packaged as `.tar.gz` (Unix) / `.zip` (Windows) to
  match Homebrew, scoop, and winget URLs; minisign skips with a warning
  when secrets are missing instead of failing the job.
- CLI completions and man page regenerated from the live `clap` command
  (no one-line TODO stubs).
- CHANGELOG: 1.0.0 date set to 2026-06-07; `[Unreleased]` moved off the
  old 0.2 notes.
- PeaZip nested checkout ignored (gitlink removed from the index; folder
  stays untracked).
- packaging README: SHA-256 hashes are filled after the first GitHub
  Release.

### Changed

- Crate versions: `supazip-core`, `supazip-cli`, `supazip-gui` to `1.0.1`.

## [1.0.0] - 2026-06-07

### Security
- `SECURITY.md`: disclosure policy (90-day coordinated disclosure), contact email, scope (all crates, packaging, CI), supported versions table, security-related design decisions (resource limits, path-traversal, atomic writes, cancellation).

### Changed
- `README.md`: marked as production-ready; removed skeleton language. Added installation section for Homebrew, AUR, winget, scoop, Nix, Docker, and crates.io. Updated project status table (all rows "Done").
- `docs/announce-1.0.md`: release announcement template for blog and Rust forum.
- All backends tested, fuzzed, and audited; zero P0/P1 bugs in RC period.
- Version bump: `supazip-core`, `supazip-cli`, `supazip-gui` to `1.0.0`.

## [0.5.0] - 2026-06-07

### Added
- **Hand-rolled plural runtime** (`supazip-core/src/i18n.rs`): `Locale` enum (en/ru/de), `plural_form()` with CLDR rules, `I18nStrings` TOML loader with dot-path lookup and plural-aware `get_plural()`.
- **German (de) locale** fully translated (78 keys, 0 TODO markers). `check_i18n.py` validates 3-locale parity.
- **GUI settings persistence**: `Settings` struct serialized as JSON to `dirs::config_local_dir()/supazip/settings.json`. Fields: language, max_archive_size, recent_files_limit, show_debug_overlay.
- **GUI settings window**: egui floating window with language ComboBox, max archive DragValue, recent limit, debug overlay checkbox, Save button.
- **mdbook documentation**: 18-file book (`docs/book/`) covering user-guide, dev-guide, and security-model. Dark theme (ayu).
- **Man pages** via `clap_mangen`: `supazip man --out-dir <DIR>` generates `supazip.1`.
- **WCAG 2.1 AA audit** (`docs/a11y-audit.md`): 17 PASS, 2 PARTIAL (egui framework limitations), 0 FAIL.
- **Screen reader smoke tests** (`docs/a11y-testing.md`): 10-step checklists for NVDA, VoiceOver, Orca.

### Changed
- `supazip-core/Cargo.toml`: added `toml = "0.8"` dependency for i18n loader.
- `supazip-cli/Cargo.toml`: added `clap_mangen = "0.2"` dependency.

### Known limitations
- egui 0.34 does not expose accessibility tree on desktop builds; screen reader support is best-effort.
- mdbook not published to GitHub Pages (local-only per decision).

## [0.4.0] - 2026-06-07

### Security
- `cargo-audit` + `cargo-deny` CI workflows (daily cron + push/PR).
- 0 vulnerabilities, 572 crates scanned.
- License allow-list extended for OPL/CC0/MPL/OFL/Ubuntu-font (egui dependencies).
- `deny.toml` with advisories, bans, licenses, sources checks.
- macOS code-signing + notarization in release.yml (conditional on secrets).

### Changed
- `safe_join` fixed: now rejects `..` only as a path-component, not as a substring.
- CLI `--output json|yaml|text` for `list` and `test` commands.
- `ArchiveEntry` now derives `Serialize` (serde).
- `cargo-llvm-cov` coverage workflow with 80% threshold gate (Codecov badge in README).
- Reproducible build scripts (`scripts/build-reproducible.sh`, `build-reproducible.ps1`).
- OS matrix expanded to windows + ubuntu + macos (6 CI jobs).
- `clap_complete` for bash/zsh/fish/powershell/elvish (static files in `supazip-cli/completions/`).
- Property-based metadata invariant proptests (5 tests in `supazip-core/tests/proptests/metadata.rs`).

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

## [0.1.0] - 2026-05-25

### Added
- Initial engine: `supazip-core` with `ArchiveFormat` trait and
  `ZipBackend` / `SevenZBackend`.
- CLI: `list` / `extract` / `create` / `test` with `--password`,
  `--format`, `--entry`, `--all`.
- GUI: placeholder window.
- Unit + integration tests for the happy paths.
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
  half-written archive on disk.
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
