# SupaZip

A cross-platform archive manager for **7z** and **ZIP**, written in Rust.
The same engine powers a desktop GUI and a CLI; both front-ends behave
identically because they share a single core crate.

> Status: the engine, CLI, automated tests, and CI entry point are working.
> The GUI is a working skeleton (toolbar, file list, status bar) backed by
> the same engine. See [Project status](#project-status) below.

## Features

- **List** archive contents (name, size, compressed size, encryption flag,
  modification time, compression method).
- **Extract** all entries or a named subset, with optional password.
  Zip-slip / 7z-slip style path-traversal attacks are refused before any
  bytes hit the filesystem.
- **Create** new archives from files on disk, in 7z or ZIP, with optional
  password. ZIP writes are AES-256 (AE-2 vendor version, the same flavour
  7-Zip produces); 7z writes are AES-256 + LZMA2.
- **Test** archive integrity (CRCs and per-entry validation).
- Resource limits to refuse zip-bomb-class inputs before they exhaust
  memory: `max_archive_size`, `max_entry_count`, `max_entry_size` (see
  [`Limits`](supazip/supazip-core/src/traits.rs)).
- Pluggable backend registry: add a new format by implementing the
  `ArchiveFormat` trait and registering its extension.
- Diagnostic error chain: `ArchiverError::InvalidArchive { source, .. }`
  preserves the underlying error from `zip` / `sevenz-rust` so the
  `e.source()` chain shows the real cause.

## Architecture

Cargo workspace rooted at [`supazip/`](supazip/) with three members:

| Crate           | Role                                                                                                                              |
|-----------------|-----------------------------------------------------------------------------------------------------------------------------------|
| `supazip-core`  | Engine. Pure Rust, no GUI dependencies. Defines the `ArchiveFormat` trait and ships two backends: `ZipBackend` (`zip 2.4.2`) and `SevenZBackend` (`sevenz-rust 0.6.1`). |
| `supazip-gui`   | eframe/egui desktop application. Skeleton (toolbar, file list, status bar) wired to the engine through `tokio`.                   |
| `supazip-cli`   | Command-line front-end. Drives `supazip-core` through `clap`.                                                                     |

The dependency graph is one-way: `core` ← `cli`, `core` ← `gui`. The GUI and
the CLI never import each other.

### Engine surface

```rust
pub trait ArchiveFormat: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &[&str];

    fn list(&self, reader: Box<dyn Read>, password: Option<&str>, limits: &Limits)
        -> Result<Vec<ArchiveEntry>, ArchiverError>;

    fn extract(&self, reader: Box<dyn Read>, dest_dir: &Path,
               entries: &[&str], password: Option<&str>,
               progress: &dyn ProgressCallback, limits: &Limits)
        -> Result<(), ArchiverError>;

    fn create(&self, writer: Box<dyn WriteSeek>, entries: &[PathBuf],
              options: &CreateOptions, password: Option<&str>,
              progress: &dyn ProgressCallback, limits: &Limits)
        -> Result<(), ArchiverError>;

    fn test(&self, reader: Box<dyn Read>, password: Option<&str>,
            progress: &dyn ProgressCallback, limits: &Limits)
        -> Result<bool, ArchiverError>;
}
```

`extract` takes a destination **directory** path (`&Path`), not a
`Box<dyn WriteSeek>`. The backends write each entry under `dest_dir` after
running the entry name through the zip crate's `enclosed_name` /
`SevenZBackend::safe_join` path-traversal guard. This kills the legacy
`chdir` hack and the `NullDest` shim that the previous CLI used to paper
over the bug.

Methods take `Box<dyn Read>` / `Box<dyn WriteSeek>` rather than the generic
`R: Read, W: Write + Seek` shown in `plan.md`. This is required for object
safety: the backends live in a `LazyLock<HashMap<&str, &'static dyn ArchiveFormat>>`
registry and are dispatched through `&dyn`. The trade-off is documented in
`supazip-core/src/traits.rs` (see the `// design note:` block).

Three `ProgressCallback` implementations ship with the engine:
`NoOpProgress`, `ProgressState` (shared atomic state), and `ChannelProgress`
(pushes updates through an `mpsc::Sender`).

## Tech stack (pinned in `Cargo.lock`)

| Component       | Crate                | Resolved version       |
|-----------------|----------------------|------------------------|
| 7z backend      | `sevenz-rust`        | 0.6.1 (feature `aes256`) |
| ZIP backend     | `zip`                | 2.4.2 (feature `chrono`) |
| GUI             | `egui` / `eframe`    | 0.34.1                 |
| File dialogs    | `rfd`                | 0.17                   |
| CLI parser      | `clap`               | 4.6 (derive)           |
| Async           | `tokio`              | 1.50                   |
| Errors          | `thiserror`          | 2                      |
| Logging         | `tracing`            | 0.1                    |
| Time            | `chrono`             | 0.4 (serde)            |
| Dev (tests)     | `tempfile`           | 3                      |

> The `plan.md` file lists `zip 8.4.0`. That is wrong — the project uses
> `zip 2.x` and the lockfile resolves to 2.4.2. The plan is the source of
> truth for *intent*, not for actual dependency versions.

## Quickstart

```bash
# Build the core and CLI (the GUI requires rustc >= 1.92; see notes below).
cd supazip
cargo build -p supazip-core -p supazip-cli

# Run the CLI:
cargo run -p supazip-cli -- --help
cargo run -p supazip-cli -- list path/to/archive.zip
cargo run -p supazip-cli -- extract path/to/archive.7z --out ./out --password secret
cargo run -p supazip-cli -- create path/to/new.zip file1.txt file2.txt
cargo run -p supazip-cli -- test path/to/archive.7z

# GUI:
cargo run -p supazip-gui
```

## Tests

```bash
# Core unit + integration tests, and CLI round-trip tests:
cd supazip
cargo test -p supazip-core -p supazip-cli

# Strict lint (clean under -D warnings):
cargo clippy -p supazip-core -p supazip-cli --no-deps -- -D warnings
cargo fmt --all -- --check

# Or use the unified local CI entry point:
scripts/ci.sh                   # POSIX
.\scripts\ci.ps1                # Windows PowerShell
```

The `ci.sh` / `ci.ps1` scripts run `build`, `test`, `fmt --check`, and
`clippy -D warnings` in order, on the same set of crates the GitHub Actions
workflow exercises.

## Security & production notes

- **Resource limits**: every backend respects `Limits::max_archive_size`
  via a `Read::take()`-bounded reader, refuses to enumerate more than
  `Limits::max_entry_count` entries, and refuses to write a single entry
  past `Limits::max_entry_size`. The CLI composes defaults with the
  `SUPAZIP_MAX_ARCHIVE_SIZE` environment variable (decimal bytes, with
  optional `K`/`M`/`G` suffix); a value like `SUPAZIP_MAX_ARCHIVE_SIZE=200M`
  caps the read at 200 MiB.
- **Path-traversal**: `SevenZBackend::safe_join` rejects empty names,
  absolute paths, and `..` components. The zip crate's `enclosed_name()`
  does the same on the ZIP side. Both backends are tested for "no escape"
  behaviour.
- **Encrypted ZIP create**: ZIP writes use `FileOptions::with_aes_encryption`
  with the AE-2 vendor version. The zip crate's blanket-on-static lifetime
  forces us to use the generic `FileOptions<'_, ()>` rather than the
  `SimpleFileOptions = FileOptions<'static, ()>` alias; the comment in
  `formats/zip.rs` documents why.
- **Encrypted 7z create**: 7z writes use `sevenz-rust::AesEncoderOptions`
  combined with LZMA2 via `set_content_methods`. The previous
  `let _ = password;` bug is fixed and tested.
- **Atomic `create`**: the CLI writes through a `tempfile::NamedTempFile`
  in the same directory and `persist`s (renames) over the target path.
  Partial writes can never replace a previous good archive at the same
  path.
- **Error chain**: `ArchiverError::InvalidArchive` /
  `UnsupportedFormat` are struct variants carrying the original error
  through `#[source]`. `eprintln!("error: {err}")` shows the message; a
  custom logger can walk the chain via `std::error::Error::source()`.
- **Cancellation**: backends call `progress.is_cancelled()` between
  entries. The CLI's exit code 130 on `ArchiverError::Cancelled` matches
  the POSIX convention for SIGINT.

## Project status

| Area                | State                                                                                                                                                  |
|---------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------|
| Engine — list       | Done for ZIP and 7z. Refuses oversized entries / counts up front via `Limits`.                                                                          |
| Engine — extract    | Done. Writes under a destination directory (`&Path`) instead of touching CWD. `safe_join` / `enclosed_name` defeat zip-slip on the entry-name side.    |
| Engine — create     | Done. ZIP honours `--password` (AES-256 AE-2). 7z honours `--password` (AES-256 + LZMA2). Atomic via `NamedTempFile` + `persist` in the CLI.           |
| Engine — test       | Done. Surfaces `ArchiverError::Cancelled` cleanly via `map_sevenz_error`.                                                                              |
| CLI                 | Done. `list` / `extract` / `create` / `test` with `--password`, `--entry`, `--format`, `--compression`. Exit 130 on `Cancelled`. Tracing env-filter.  |
| GUI                 | Working skeleton. Toolbar (Open / Extract / Create / Test), file list (when an archive is open), status bar. See `supazip-gui/src/main.rs`.             |
| CI                  | Local entry point `scripts/ci.sh` / `scripts/ci.ps1`. GitHub Actions workflow at `.github/workflows/ci.yml` (Windows runner).                           |
| Tests               | 45 core unit tests + 12 CLI integration tests. Coverage spans list, extract, test, create, encryption, error variants, limits, cancellation, traversal. |
| Lockfile            | Committed.                                                                                                                                             |
| `.gitignore`        | Present at repo root.                                                                                                                                  |

## Repository layout

```
SupaZip/
├── README.md              # this file
├── plan.md                # original design plan (intentionally out of date in places)
├── AGENTS.md              # Cursor agent roles and memory-bank pointer
├── CHANGELOG.md           # per-release changes
├── .github/
│   └── workflows/
│       └── ci.yml         # GitHub Actions: build, test, fmt, clippy on windows-latest
├── scripts/
│   ├── ci.sh              # POSIX local CI entry point
│   └── ci.ps1             # Windows PowerShell counterpart
├── .cursor/               # Cursor rules, skills, MCP config
│   ├── rules/             # 5 role rules (stack, architect, code, debug, ask)
│   ├── skills/            # workspace map + memory-bank workflow
│   └── mcp.json           # Context7 MCP
├── memory-bank/           # living project context (productContext, activeContext, …)
└── supazip/               # Cargo workspace
    ├── Cargo.toml
    ├── supazip-core/
    ├── supazip-gui/
    └── supazip-cli/
```

## Memory bank

`memory-bank/` holds the project context that survives across sessions:
`productContext.md`, `activeContext.md`, `systemPatterns.md`,
`decisionLog.md`, `progress.md`, `projectBrief.md`, and `architect.md`. The
schema and update workflow live in
[`.cursor/skills/supazip-memory-bank/SKILL.md`](.cursor/skills/supazip-memory-bank/SKILL.md).

## License

Dual-licensed under MIT or Apache-2.0, at your option. See `LICENSE-MIT` /
`LICENSE-APACHE` (generated by the licensing step in `memory-bank/decisionLog.md`).
