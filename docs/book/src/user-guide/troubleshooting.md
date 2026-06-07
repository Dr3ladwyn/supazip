# Troubleshooting

## Common errors

### `error: unsupported format: <ext>`

The file extension is not recognised by the backend registry.

**Fix:** Use `--format zip` or `--format 7z` to override detection. Ensure the file has a recognised extension (`.zip`, `.7z`, `.tar`, `.tar.gz`, `.tgz`, `.tar.xz`, `.txz`).

### `error: Password required`

The archive is encrypted and no `--password` flag was provided.

**Fix:** Add `--password <your-password>` to the command. In the GUI, the password dialog opens automatically.

### `error: Wrong password`

The provided password does not match the archive's encryption key.

**Fix:** Verify the password. For 7z archives with header encryption, the password is needed to even list the contents.

### `error: Archive exceeds resource limit: <details>`

The archive exceeds one of the configured resource limits (size, entry count, entry size, or compression ratio).

**Fix:** Increase the limit via `SUPAZIP_MAX_ARCHIVE_SIZE` or review the [Resource limits](../security-model/resource-limits.md) chapter.

### `error: unsafe path: path traversal in <entry>`

An archive entry contains `..` path components that would escape the extraction directory.

**Fix:** The archive is likely crafted maliciously. Do not extract it. See [Path traversal](../security-model/path-traversal.md) for details.

### `warning: ignoring SUPAZIP_MAX_ARCHIVE_SIZE='<raw>'`

The environment variable value could not be parsed.

**Fix:** Use a decimal byte count or a suffix: `512M`, `2G`, `1073741824`.

### `error: IO error: <msg>`

A filesystem I/O error occurred (permission denied, disk full, file not found).

**Fix:** Check file permissions, disk space, and that the path exists.

### `error: cannot detect format from '<path>' (no extension); pass --format`

The archive has no file extension and `--format` was not provided.

**Fix:** Add `--format zip` or `--format 7z`.

## MSRV (Minimum Supported Rust Version)

SupaZip targets **Rust 1.92** as its MSRV. This is enforced in CI and declared in each crate's `Cargo.toml`:

```toml
[package]
rust-version = "1.92"
```

If you see compilation errors on a newer Rust version, ensure your toolchain is up to date:

```bash
rustup update stable
```

If you need to build with exactly 1.92:

```bash
rustup toolchain install 1.92
cargo +1.92 build --release
```

## Toolchain setup

### Windows

Install the MSVC build tools:
1. Download [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
2. Select "Desktop development with C++".
3. Install and restart your terminal.

Then install Rust via [rustup.rs](https://rustup.rs/).

### Linux (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install build-essential curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### macOS

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Build failures

### `error: linking with cc failed`

A C linker is not available. Install the platform-specific build tools (see above).

### `error: failed to run custom build command for 'lzma-rs'`

`lzma-rs` is pure Rust and should not require a C toolchain. If this error appears, run `cargo clean` and retry. If the issue persists, check that `Cargo.lock` is not corrupted.

### `error: no matching package found`

A dependency version conflict. Run `cargo update` to refresh `Cargo.lock`, then retry.

## GUI issues

### Window does not appear (Linux)

Some Wayland compositors do not support the egui backend. Try running with `WINIT_UNIX_BACKEND=x11`:

```bash
WINIT_UNIX_BACKEND=x11 supazip-gui
```

### Drag and drop does not work (Wayland)

Full Wayland DnD support is deferred to 1.1. Use **File → Open** as a fallback.

## Performance

### Slow extraction of large archives

SupaZip uses streaming I/O with a 64 KiB buffer. For very large archives (>1 GiB), extraction time is dominated by disk I/O. Ensure the destination is on a fast disk.

### High memory usage during `create`

The `create` command buffers the entire archive in memory before writing to disk (to support atomic writes). For very large archives, ensure sufficient RAM is available.
