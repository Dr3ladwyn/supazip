# Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-05 | Use `Box<dyn Read>` / `Box<dyn WriteSeek>` in `ArchiveFormat`, not `<R: Read, W: Write + Seek>` generics. | Object safety is required for the `BACKENDS: LazyLock<HashMap<&str, &dyn ArchiveFormat>>` registry. Documented in `traits.rs` design note. |
| 2026-06-05 | Enable `zip/chrono` feature and use `TryFrom<zip::DateTime> for chrono::NaiveDateTime`. | `zip 2.4.2` removed `try_to_datetime`. Direct `chrono` conversion is the supported path. |
| 2026-06-05 | Replace `ZipArchive::set_password` with `by_index_decrypt` / `by_name_decrypt`. | `set_password` was removed in `zip 2.x`; per-entry decrypt API is the current idiom. |
| 2026-06-05 | Use `SevenZWriter::set_content_methods([Aes256Sha256 + LZMA2])` for encrypted 7z create, instead of returning `Unsupported`. | The library **does** support it; the original `let _pwd = password` was a real bug, not an upstream limitation. |
| 2026-06-05 | Keep `extract` buffered in core, but `list` and `test` stream. | `sevenz-rust 0.6.1` and `zip 2.4.2` both need a `Read + Seek` input internally; streaming the outer pass saves no copies. Code comment in `formats/zip.rs` and `formats/sevenz.rs` explains. |
| 2026-06-05 | Report 7z `ArchiveEntry::encrypted` based on the entry's `has_crc` / folder-coder state, not hard-coded `false`. | Plaintext 7z entries expose this through the coder type; the previous hard-coded `false` was a real bug. |
| 2026-06-05 | `git init` the repo root before Phase 4 commits. | The directory was never a git repo; the brief asks for logical chunks of commits and a final report. |
| 2026-06-05 | GUI build is intentionally left out of `cargo build` / `cargo test` runs in CI / verification. | egui 0.34.1 needs rustc 1.92; local toolchain is 1.88.0. This is a toolchain issue, not a code issue. |
