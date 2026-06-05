# SupaZip

A cross-platform archive manager for **7z** and **ZIP**, written in Rust.
The same engine powers a desktop GUI and a CLI; both front-ends behave
identically because they share a single core crate.

> Status: the engine and CLI are working and tested end-to-end. The GUI is a
> placeholder window. See [Project status](#project-status) below.

## Features

- **List** archive contents (name, size, compressed size, encryption flag,
  modification time, compression method).
- **Extract** all entries or a named subset, with optional password.
- **Create** new archives from files on disk, in 7z or ZIP, with optional
  password (AES-256 for 7z).
- **Test** archive integrity (CRCs and per-entry validation).
- Pluggable backend registry: add a new format by implementing the
  `ArchiveFormat` trait and registering its extension.

## Architecture

Cargo workspace rooted at [`supazip/`](supazip/) with three members:

| Crate           | Role                                                         |
|-----------------|--------------------------------------------------------------|
| `supazip-core`  | Engine. Pure Rust, no GUI dependencies. Defines the `ArchiveFormat` trait and ships two backends: `ZipBackend` (`zip 2.4.2`) and `SevenZBackend` (`sevenz-rust 0.6.1`). |
| `supazip-gui`   | eframe/egui desktop application. Currently a placeholder.   |
| `supazip-cli`   | Command-line front-end. Drives `supazip-core` through `clap`. |

The dependency graph is one-way: `core` ← `cli`, `core` ← `gui`. The GUI and
the CLI never import each other.

### Engine surface

```rust
pub trait ArchiveFormat: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &[&str];

    fn list(&self, reader: Box<dyn Read>, password: Option<&str>)
        -> Result<Vec<ArchiveEntry>, ArchiverError>;

    fn extract(&self, reader: Box<dyn Read>, dest: Box<dyn WriteSeek>, entries: &[&str],
               password: Option<&str>, progress: &dyn ProgressCallback)
        -> Result<(), ArchiverError>;

    fn create(&self, writer: Box<dyn WriteSeek>, entries: &[PathBuf],
              options: &CreateOptions, password: Option<&str>,
              progress: &dyn ProgressCallback)
        -> Result<(), ArchiverError>;

    fn test(&self, reader: Box<dyn Read>, password: Option<&str>,
            progress: &dyn ProgressCallback)
        -> Result<bool, ArchiverError>;
}
```

Methods take `Box<dyn Read>` / `Box<dyn WriteSeek>` rather than the generic
`R: Read, W: Write + Seek` shown in `plan.md`. This is required for object
safety: the backends live in a `LazyLock<HashMap<&str, &'static dyn ArchiveFormat>>`
registry and are dispatched through `&dyn`. The trade-off is documented in
`supazip-core/src/traits.rs` (see the `// design note:` block).

Three `ProgressCallback` implementations ship with the engine:
`NoOpProgress`, `ProgressState` (shared atomic state), and `ChannelProgress`
(pushes updates through an `mpsc::Sender`).

## Tech stack (pinned in `Cargo.lock`)

| Component       | Crate                | Resolved version |
|-----------------|----------------------|------------------|
| 7z backend      | `sevenz-rust`        | 0.6.1 (feature `aes256`) |
| ZIP backend     | `zip`                | 2.4.2 (feature `chrono`) |
| GUI             | `egui` / `eframe`    | 0.34.1           |
| File dialogs    | `rfd`                | 0.17             |
| CLI parser      | `clap`               | 4.6 (derive)     |
| Async           | `tokio`              | 1.50             |
| Errors          | `thiserror`          | 1.0.69           |
| Logging         | `tracing`            | 0.1              |
| Time            | `chrono`             | 0.4 (serde)      |
| Dev (tests)     | `tempfile`           | 3                |

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
```

## Tests

```bash
# Core unit + integration tests, and CLI round-trip tests:
cd supazip
cargo test -p supazip-core -p supazip-cli

# Lint (clean under -D warnings as of this writing):
cargo clippy -p supazip-core -p supazip-cli --all-targets -- -D warnings
```

Current state: **6 core tests + 3 CLI integration tests, all passing.** The
CLI integration tests build a real ZIP, then drive the compiled
`supazip-cli` binary through `list` / `extract` / `test` and assert on the
output.

## Project status

| Area                  | State                                                                          |
|-----------------------|--------------------------------------------------------------------------------|
| Engine — list         | Done for ZIP and 7z.                                                            |
| Engine — extract      | Done. **Known issue**: the core's `extract` writes through `enclosed_name` to the *current working directory* and ignores the `Box<dyn WriteSeek>` argument. The CLI emulates `--out` by `chdir`-ing for the duration of the call. A future commit will thread a directory path (or a tar stream) through the trait properly. |
| Engine — create       | Done for both formats. 7z honours `--password` (AES-256 + LZMA2). ZIP does not yet — `zip 2.x` supports it via `SimpleFileOptions::with_password`, the API is wired through `CreateOptions` shape but the implementation is left as a follow-up. |
| Engine — test         | Done for both formats.                                                          |
| Engine — streaming    | `list` and `test` stream from the reader where possible. ZIP needs the central directory and so buffers once (documented in `formats/zip.rs`). 7z uses a single `SharedBuffer` for any second pass and never clones the archive bytes. |
| Engine — encryption   | 7z `ArchiveEntry::encrypted` is now derived from the folder coders (was hard-coded `false`). |
| CLI                   | Done. `list` / `extract` / `create` / `test` with `--password`, `--all`, `--entry`, `--format`. |
| GUI                   | **Not done.** Single placeholder window. Blocked locally on rustc 1.88 (egui 0.34.1 needs 1.92). |
| CI                    | Not configured.                                                                 |
| Lockfile              | Committed.                                                                       |
| `.gitignore`          | Present at repo root.                                                            |

## Repository layout

```
SupaZip/
├── README.md              # this file
├── plan.md                # original design plan (intentionally out of date in places)
├── AGENTS.md              # Cursor agent roles and memory-bank pointer
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

Not yet decided. Add a `LICENSE` file before publishing.
