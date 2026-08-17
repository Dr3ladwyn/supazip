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
- **Design tokens at GUI runtime.** `supazip-gui` embeds
  `design/tokens.json` via `include_str!` and maps `themes.dark` /
  `themes.light` to `egui::Style` through `theme::Style::from_tokens`.
  Settings persist `theme: Dark | Light | System`. `supazip-core` stays
  free of egui. Toolbar/status use elevation `level_1`; dialogs use
  `level_2`.

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
- CLI `list` text layout is loaded from `design/cli-table.json` (mirror of
  `design/cli-table.tera`). Optional ANSI color uses `themes.dark` from
  `design/tokens.json` only when stdout is a TTY (`anstyle` +
  `anstyle-query`). `--output json|yaml` stays uncoloured.
- Cancellation is polled once per buffer fill (8 KiB) inside loops, not
  on every entry, to keep overhead negligible.
