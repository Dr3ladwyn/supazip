# SupaZip: System Architecture

> Living snapshot of the architecture **as the code is right now**.
> The original design intent is in `plan.md` at the repo root. The two do not
> always agree; when they disagree, follow the code.

## Workspace

- Cargo workspace rooted at `supazip/` with three members: `supazip-core`,
  `supazip-gui`, `supazip-cli`. Workspace manifest: `supazip/Cargo.toml`.
- `resolver = "2"`, Rust edition 2021.

## supazip-core (the engine)

The engine is GUI-free by design. Public surface lives in
`supazip-core/src/lib.rs`:

- `ArchiverError` — `thiserror` enum: `Io`, `InvalidArchive`, `PasswordRequired`,
  `WrongPassword`, `Cancelled`, `UnsupportedFormat`.
- `ArchiveFormat` trait — `name`, `extensions`, `list`, `extract`, `create`, `test`.
  All `Read`/`WriteSeek` parameters are boxed `dyn` for object safety (see
  `traits.rs` design note).
- `ArchiveEntry` — name, path, is_dir, size, compressed_size, modified, method,
  crc32, encrypted.
- `CreateOptions` — compression method + level.
- `ProgressCallback` — `set_progress`, `set_message`, `is_cancelled`. Three
  implementations live in `traits.rs`: `NoOpProgress`, `ProgressState` (shared
  atomic state, e.g. for a GUI thread), `ChannelProgress` (sends
  `ProgressUpdate` through an `mpsc::Sender`).
- `WriteSeek` — local `Write + Seek` supertrait used by the trait object.
- Backends: `ZipBackend` (`zip 2.4.2`), `SevenZBackend` (`sevenz-rust 0.6.1`).
- Registry: `BACKENDS: LazyLock<HashMap<&str, &'static dyn ArchiveFormat>>` in
  `formats/mod.rs`. Lookup by lowercase file extension via `get_backend`.
- Note: `detect_format` accepts a reader but currently only does extension
  matching; the `reader` parameter is unused (warning lives there).

## supazip-gui

Stub: `eframe::run_native` opens a window that says "Coming soon…". The
PeaZip-style layout promised in `plan.md` (toolbar / tabs / address bar /
filelist / status bar) is **not** implemented. Blocked at build time on this
machine by egui 0.34.1 needing rustc 1.92 (local toolchain is 1.88.0).

## supazip-cli

Drives `supazip-core` from a `clap` derive subcommand enum. Each subcommand
resolves a backend via `supazip_core::formats::get_backend` based on the
archive's extension, then dispatches to the trait method.

## Boundaries

- Core must not depend on `egui`, `eframe`, or `rfd`.
- GUI and CLI are both consumers of core; never the other way around.
- All cross-thread state in core goes through `ProgressState` (atomics +
  Mutex<String>), not raw channels, except `ChannelProgress` which is
  explicitly for "push to UI thread" callers.

## Streaming vs buffered I/O

- `list` and `test` operate on the reader directly: the zip backend uses
  `ZipArchive::new(reader)` (which wraps in `RefReader` for the cases that
  need seek) and the 7z backend uses `SevenZReader::new(reader, u64::MAX, …)`,
  which performs one buffering pass internally for its own seek needs.
- `extract` and `create` need `Write + Seek` and pass a single
  `Box<dyn WriteSeek>` end-to-end, with no per-entry full-archive copy.

## Build-time caveats

- `zip 2.x` removed `set_password` and `try_to_datetime`; we use
  `by_index_decrypt` / `by_name_decrypt` and the `zip/chrono` feature
  (which converts `zip::DateTime` into `chrono::NaiveDateTime`).
- `sevenz-rust 0.6.1` exposes `set_content_methods` on `SevenZWriter`, which
  is what we use to apply AES-256 + LZMA2 for encrypted 7z creation.
