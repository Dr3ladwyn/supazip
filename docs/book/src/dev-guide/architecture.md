# Architecture

SupaZip is a Cargo workspace with three crates. The core engine is format-agnostic; the CLI and GUI are thin front-ends.

## Dependency graph

```
supazip-cli  ──┐
               ├──▶  supazip-core  ──▶  sevenz-rust, zip, tar, flate2, lzma-rs
supazip-gui  ──┘         ▲
                         │
                    thiserror, serde, chrono, tokio, log
```

| Crate | Dependencies | Role |
|-------|-------------|------|
| `supazip-core` | `sevenz-rust`, `zip`, `tar`, `flate2`, `lzma-rus`, `thiserror`, `serde`, `chrono`, `tokio`, `log` | Archive engine |
| `supazip-cli` | `supazip-core`, `clap`, `clap_complete`, `tracing`, `tracing-subscriber`, `tempfile`, `serde_json`, `serde_yaml` | CLI front-end |
| `supazip-gui` | `supazip-core`, `eframe`, `egui`, `rfd`, `tokio`, `log` | Desktop GUI |

## Core traits

The engine surface is defined by two traits in `supazip-core/src/traits.rs`:

### `ArchiveFormat`

Every backend implements this trait. It is object-safe so backends can be stored as `&'static dyn ArchiveFormat` in the global registry.

```rust
pub trait ArchiveFormat: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &[&str];

    fn list(&self, reader: Box<dyn Read>, password: Option<&str>, limits: &Limits)
        -> Result<Vec<ArchiveEntry>, ArchiverError>;

    fn extract(&self, reader: Box<dyn Read>, dest_dir: &Path, entries: &[&str],
        password: Option<&str>, progress: &dyn ProgressCallback, limits: &Limits)
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

### `ProgressCallback`

Progress and cancellation are decoupled from the UI. Three implementations exist:

| Implementation | Used by | Cancellable |
|---------------|---------|-------------|
| `NoOpProgress` | Tests, CLI (quiet mode) | No |
| `Arc<ProgressState>` | GUI, CLI (via shared state) | Yes |
| `ChannelProgress` | GUI (mpsc channel to UI thread) | No |

### `WriteSeek`

A local supertrait `Write + Seek` that exists so trait methods can name a single type without colliding with std types. Every `T: Write + Seek` implements it via a blanket impl.

## Backend registry

Backends are registered in a global `LazyLock<HashMap>` in `supazip-core/src/formats/mod.rs`:

```rust
pub static BACKENDS: LazyLock<HashMap<&'static str, &'static dyn ArchiveFormat>> = ...
```

The registry maps file extensions (e.g. `"zip"`, `"7z"`, `"tar.gz"`) to `&'static dyn ArchiveFormat`. Lookup is case-insensitive. The `get_backend(ext)` function is the primary entry point for both CLI and GUI.

## Error model

All engine methods return `Result<_, ArchiverError>`. The enum (defined with `thiserror`) has these variants:

| Variant | Meaning |
|---------|---------|
| `Io(io::Error)` | Filesystem / network I/O failure |
| `InvalidArchive { message, source }` | Corrupt or malformed archive data |
| `PasswordRequired` | Encrypted archive, no password supplied |
| `WrongPassword` | Password supplied but incorrect |
| `Cancelled` | Operation interrupted via `ProgressCallback::is_cancelled` |
| `UnsupportedFormat { message }` | Extension not in the registry |
| `TooLarge(String)` | Resource limit exceeded |

## Trait diagram

```
ArchiveFormat (object-safe)
├── name(), extensions()
├── list()       → Vec<ArchiveEntry>
├── extract()    → ()
├── create()     → ()
└── test()       → bool

ProgressCallback: Send
├── set_progress(current, total)
├── set_message(msg)
└── is_cancelled() → bool

WriteSeek: Write + Seek
└── blanket impl for T: Write + Seek
```

## GUI state machine

The GUI runs an `AppController` that owns an `AppState` and communicates with worker threads via `mpsc` channels:

```
GUI thread (egui)  ◀── EngineEvent ──  Worker thread
       │                                    │
       ▼                                    │
  AppController.apply()                     │
       │                                    │
       ├── Listed   → update grid           │
       ├── Done     → clear busy            │
       ├── Error    → show error            │
       └── PasswordRequired → open dialog   │
                                            │
  AppController.cancel()  ──cancel_flag──▶  │
```

Workers are spawned as `std::thread::spawn` closures that call into `supazip-core` synchronously. The GUI drains the event channel once per frame.

## Design documents

- **Design system:** [`DESIGN.md`](../../../DESIGN.md) — tokens, layout, motion, i18n, accessibility.
- **Implementation plan:** [`plan.md`](../../../plan.md) — workspace layout, core traits, GUI layout, implementation order.
- **Roadmap:** [`ROADMAP.md`](../../../ROADMAP.md) — milestones 0.2.0 through 1.0.0, track assignments.
