# System Patterns

## Architectural patterns

- **Shared engine, two front-ends.** `supazip-core` exposes the
  `ArchiveFormat` trait; GUI and CLI are thin consumers. No GUI types in
  core, no CLI types in core, no GUI types in CLI.
- **Object-safe trait objects for the backend registry.** The
  `ArchiveFormat` trait is consumed as `&'static dyn ArchiveFormat` in
  the `BACKENDS: LazyLock<HashMap<&str, &dyn ArchiveFormat>>` registry, so
  methods take `Box<dyn Read>` / `Box<dyn WriteSeek>`. Trade-off: we give
  up monomorphization to keep the registry simple. Documented in
  `traits.rs` `// design note:` block.
- **Progress as a first-class concern.** Every long-running trait method
  takes `&dyn ProgressCallback` and the engine ships three implementations
  (`NoOpProgress`, `ProgressState`, `ChannelProgress`) so callers can pick
  the model that matches their UI.

## Design patterns

- **Backend per extension.** Each format backend implements
  `ArchiveFormat` and registers its extensions in `BACKENDS`. Lookup is
  just `get_backend(ext)`. Adding a new format = implementing the trait
  and a one-line insert.
- **`ArchiverError` as the single error type.** `thiserror` enum with
  `#[from] io::Error` so `?` works at every call site. Format-specific
  errors (`ZipError`, `SevenZError`) are mapped into `ArchiverError` at
  the boundary in each backend.

## Common idioms

- Buffered copy into a `Vec<u8>` only when the upstream library truly
  needs `Seek` on the input. Otherwise the trait method takes the reader
  directly and uses 8 KiB / 4 KiB read loops with cancellation polling.
- For `extract`, parent directories are created implicitly by
  `std::fs::File::create` on the destination path. We do not pre-create
  them explicitly; the OS does the right thing on Windows and Unix.
- `tracing::info!` / `tracing::debug!` at the start and end of every
  trait method, with the operation name and entry count.
- Cancellation is polled once per buffer fill (8 KiB) inside loops, not
  on every entry, to keep overhead negligible.
