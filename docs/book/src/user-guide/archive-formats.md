# Archive formats

SupaZip supports five archive formats through a pluggable backend registry. Each format is implemented as a struct that satisfies the `ArchiveFormat` trait in `supazip-core`.

## ZIP

| Property | Value |
|----------|-------|
| Extension | `.zip` |
| Library | [`zip`](https://crates.io/crates/zip) 2.4 |
| Encryption | AES-256 (AE-2) |
| Read / Write | Yes / Yes |

### Compression methods

| Method | CLI flag | Description |
|--------|----------|-------------|
| Deflate | `--compression deflate` | The classic ZIP default. Universally compatible. |
| Store | `--compression store` | No compression. Bytes are written verbatim. `compressed_size == size` for every entry. |
| Zstd | `--compression zstd` | Zstandard — fast, dictionary-friendly. Written through the `zip` crate's `zstd` feature. |
| Brotli | `--compression brotli` | High-ratio text compression. Implemented as a Stored entry with a `.br` filename suffix; decompressed transparently on extract. |

### Notes

- ZIP is the default format when the file extension is `.zip`.
- The ZIP backend supports `brotli` and `zstd` as compression methods, which are added in milestone 0.2.0 through the `zip` crate's native feature flags.
- AES-256 encryption uses the AE-2 scheme. The password is required at create time and at extract/list time.

## 7z

| Property | Value |
|----------|-------|
| Extension | `.7z` |
| Library | [`sevenz-rust`](https://crates.io/crates/sevenz-rust) 0.6 |
| Encryption | AES-256 + LZMA2 |
| Read / Write | Yes / Yes |

### Compression

7z always uses LZMA2 internally. The compression method is not configurable through the CLI `--compression` flag; it is determined by the backend:

- **Without password:** LZMA2 only.
- **With password:** AES-256 encryption wrapping LZMA2-compressed data.

### Notes

- 7z is the only format that supports header encryption (the entire archive header is encrypted, hiding the entry list).
- The `sevenz-rust` crate is pure Rust with no C dependencies.
- Solid archives (where multiple files are compressed together for better ratio) are supported.

## TAR

| Property | Value |
|----------|-------|
| Extension | `.tar` |
| Library | [`tar`](https://crates.io/crates/tar) 0.4 |
| Compression | None (uncompressed) |
| Encryption | No |
| Read / Write | Yes / Yes |

TAR is the uncompressed tape archive format. It stores files sequentially with POSIX headers. No compression or encryption is applied.

## TAR.GZ

| Property | Value |
|----------|-------|
| Extension | `.tar.gz`, `.tgz` |
| Library | `tar` + [`flate2`](https://crates.io/crates/flate2) 1 |
| Compression | Gzip (Deflate) |
| Encryption | No |
| Read / Write | Yes / Yes |

TAR.GZ wraps a TAR stream in gzip compression. The `flate2` crate defaults to the `miniz_oxide` Rust backend (no C dependency on Windows).

## TAR.XZ

| Property | Value |
|----------|-------|
| Extension | `.tar.xz`, `.txz` |
| Library | `tar` + [`lzma-rs`](https://crates.io/crates/lzma-rs) 0.3 |
| Compression | LZMA2 / XZ |
| Encryption | No |
| Read / Write | Yes / Yes |

TAR.XZ wraps a TAR stream in XZ (LZMA2) compression. The `lzma-rs` crate is a pure-Rust implementation chosen over the `xz2` C wrapper to keep the CI matrix simple and the binary self-contained.

## Format detection

Archive format is detected by file extension. The backend registry in `supazip-core/src/formats/mod.rs` maps extensions to backends:

| Extension | Backend |
|-----------|---------|
| `.zip` | `ZipBackend` |
| `.7z` | `SevenZBackend` |
| `.tar` | `TarBackend` |
| `.tar.gz`, `.tgz` | `TarGzBackend` |
| `.tar.xz`, `.txz` | `TarXzBackend` |

Compound extensions (e.g. `.tar.gz`) are checked before single extensions so `archive.tar.gz` does not incorrectly match a bare `.gz` lookup. Extension matching is case-insensitive.

The `--format` CLI flag overrides extension-based detection. This is useful when the file has no extension or an ambiguous one.

## Unsupported formats

RAR is explicitly out of scope for 1.0. The format is proprietary and would require `unar-bindings`; support is deferred to 1.1.
