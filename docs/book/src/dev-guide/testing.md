# Testing

SupaZip uses a layered testing strategy: unit tests in every module, property-based round-trip tests, criterion benchmarks, and cargo-fuzz harnesses.

## Unit tests

Every module in `supazip-core` and `supazip-cli` contains inline `#[cfg(test)] mod tests` blocks. Run them all:

```bash
cargo test --workspace
```

Run tests for a single crate:

```bash
cargo test -p supazip-core
cargo test -p supazip-cli
cargo test -p supazip-gui
```

Run a specific test by name:

```bash
cargo test safe_join_rejects_dotdot
```

### Key test areas

| Area | Files | What is tested |
|------|-------|----------------|
| Backend round-trips | `formats/zip.rs`, `formats/sevenz.rs`, `formats/tar.rs`, `formats/tar_gz.rs`, `formats/tar_xz.rs` | Create → list → extract → compare. Password-protected variants. |
| Path traversal | `formats/mod.rs` | `safe_join` rejects `..`, empty names, escape attempts. Accepts normal paths and `..` in filenames. |
| Resource limits | `traits.rs` | `Limits::default()` sanity, `Limits::new()` matches default, `is_unrestricted()` logic, compression ratio guard. |
| Error model | `error.rs` | Display formatting, source chain preservation, `From<io::Error>`, every variant. |
| Progress callbacks | `traits.rs` | `NoOpProgress`, `ProgressState`, `Arc<ProgressState>`, `ChannelProgress` — round-trip, cancellation, channel delivery. |
| Backend registry | `formats/mod.rs` | Extension lookup (case-insensitive), `detect_format` by extension, unknown extensions return `None`. |
| GUI state machine | `lib.rs` (supazip-gui) | `AppController::apply` for every `EngineEvent` variant, context-menu dispatch, password dialog lifecycle, progress state, recent files. |
| CLI output | `output.rs` (supazip-cli) | JSON and YAML serialization of list results and test results. |
| Completions | `completions.rs` (supazip-cli) | Shell completion generation for all five shells. |

## Property-based tests (proptest)

Proptest lives in `supazip-core/tests/proptests/`. These tests generate random inputs and assert invariants:

```rust
proptest! {
    #[test]
    fn zip_roundtrip(files in arb_file_list()) {
        // Create a ZIP, list it, extract it, compare bytes.
    }
}
```

Run proptests:

```bash
cargo test -p supazip-core --test proptests
```

### Invariants tested

- **Round-trip:** Create → extract → contents match originals.
- **Entry count:** Listed entries match the files added.
- **Size consistency:** Uncompressed size matches original file size.
- **Path safety:** No extracted path escapes the destination directory.

## Benchmarks (criterion)

Benchmarks live in `supazip-core/benches/engine.rs`. They measure:

| Operation | Format | Metric |
|-----------|--------|--------|
| `list` | ZIP, 7z, TAR, TAR.GZ, TAR.XZ | Time to list all entries |
| `extract` | All | Time to extract all entries to a temp directory |
| `create` | All | Time to create an archive from a set of files |
| `test` | All | Time to verify integrity |

Run benchmarks:

```bash
cargo bench -p supazip-core
```

Results are written to `target/criterion/`. HTML reports are generated with the `html_reports` feature.

### Storing baselines

Benchmark baselines are stored as CI artefacts. Compare against a baseline:

```bash
cargo bench -p supazip-core -- --save-baseline current
cargo bench -p supazip-core -- --baseline current
```

## Fuzzing (cargo-fuzz)

Fuzz targets live in `supazip-core/fuzz/fuzz_targets/`. Each target exercises one backend's `list`, `extract`, or `create` method with arbitrary input:

```bash
cd supazip-core
cargo fuzz run fuzz_list_zip
cargo fuzz run fuzz_extract_7z
cargo fuzz run fuzz_create_tar
```

Fuzzing runs for 24 hours in CI (milestone 0.4.0 criterion). Crashes are reported as CI failures.

## Test data

Test archives are created on-the-fly in `tempdir()`. No binary fixtures are committed to the repository. The pattern:

```rust
fn build_small_zip(path: &Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut zw = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
    zw.start_file("hello.txt", opts).unwrap();
    zw.write_all(b"hi\n").unwrap();
    zw.finish().unwrap();
}
```

## Coverage

Test coverage is measured with `cargo llvm-cov`. The CI workflow uploads a coverage badge and enforces a minimum threshold:

| Milestone | Threshold |
|-----------|-----------|
| 0.3.0 | 60% |
| 0.4.0 | 80% |
| 1.0.0 | 95% |

Generate a local coverage report:

```bash
cargo llvm-cov --workspace --html
```

The HTML report lands in `target/llvm-cov/html/index.html`.
