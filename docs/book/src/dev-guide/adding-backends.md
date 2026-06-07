# Adding backends

SupaZip's format support is pluggable. Adding a new archive format requires implementing the `ArchiveFormat` trait and registering the backend in the global registry.

## Step 1: Implement `ArchiveFormat`

Create a new file in `supazip-core/src/formats/`, e.g. `myformat.rs`:

```rust
use crate::error::ArchiverError;
use crate::traits::{ArchiveFormat, ArchiveEntry, CreateOptions, Limits, ProgressCallback, WriteSeek};
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct MyFormatBackend;

impl MyFormatBackend {
    pub fn new() -> Self { Self }
}

impl ArchiveFormat for MyFormatBackend {
    fn name(&self) -> &'static str {
        "myformat"
    }

    fn extensions(&self) -> &[&str] {
        &["mf"]
    }

    fn list(
        &self,
        reader: Box<dyn Read>,
        password: Option<&str>,
        limits: &Limits,
    ) -> Result<Vec<ArchiveEntry>, ArchiverError> {
        // Parse the archive header and return entry metadata.
        // Enforce limits.max_entry_count.
        todo!()
    }

    fn extract(
        &self,
        reader: Box<dyn Read>,
        dest_dir: &Path,
        entries: &[&str],
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<(), ArchiverError> {
        // Use safe_join() for every entry to prevent path traversal.
        // Check progress.is_cancelled() between entries.
        // Enforce limits.max_entry_size during streaming.
        todo!()
    }

    fn create(
        &self,
        writer: Box<dyn WriteSeek>,
        entries: &[PathBuf],
        options: &CreateOptions,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<(), ArchiverError> {
        // Write the archive format to the writer.
        // Report progress via set_progress / set_message.
        todo!()
    }

    fn test(
        &self,
        reader: Box<dyn Read>,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<bool, ArchiverError> {
        // Verify checksums / signatures without extracting.
        todo!()
    }
}
```

## Step 2: Register the backend

Add the module and register in `supazip-core/src/formats/mod.rs`:

```rust
mod myformat;
pub use myformat::MyFormatBackend;
```

In the `BACKENDS` LazyLock initializer, add:

```rust
let myformat: &'static MyFormatBackend = Box::leak(Box::new(MyFormatBackend::new()));
for ext in myformat.extensions() {
    map.insert(*ext, myformat as &dyn ArchiveFormat);
}
```

## Step 3: Re-export from `lib.rs`

In `supazip-core/src/lib.rs`, add `MyFormatBackend` to the `pub use formats::` line:

```rust
pub use formats::{
    detect_format, get_backend, supported_extensions,
    MyFormatBackend, SevenZBackend, ZipBackend, BACKENDS,
};
```

## Step 4: Update format detection

If the format uses a compound extension (e.g. `.tar.mf`), add it to the compound-extension check list in `detect_format()`:

```rust
for ext in ["tar.gz", "tar.xz", "tar.bz2", "tar.zst", "tar.mf"] {
    // ...
}
```

## Step 5: Add tests

### Round-trip test

```rust
#[test]
fn myformat_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("test.mf");
    let backend = MyFormatBackend::new();

    // Create
    let writer = Box::new(std::io::Cursor::new(Vec::new()));
    let files = vec![/* temp files */];
    let opts = CreateOptions::default();
    let limits = Limits::default();
    backend.create(writer, &files, &opts, None, &NoOpProgress, &limits).unwrap();

    // List
    let reader = Box::new(std::io::Cursor::new(/* archive bytes */));
    let entries = backend.list(reader, None, &limits).unwrap();
    assert_eq!(entries.len(), files.len());

    // Extract
    let reader = Box::new(std::io::Cursor::new(/* archive bytes */));
    let out = dir.path().join("out");
    backend.extract(reader, &out, &[], None, &NoOpProgress, &limits).unwrap();
    // Compare extracted files with originals.
}
```

### Property-based test

```rust
proptest! {
    #[test]
    fn myformat_roundtrip_proptest(files in arb_file_list()) {
        // Same as above but with random inputs.
    }
}
```

### Fuzz target

Create `supazip-core/fuzz/fuzz_targets/fuzz_list_myformat.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let backend = MyFormatBackend::new();
    let reader = Box::new(std::io::Cursor::new(data));
    let _ = backend.list(reader, None, &Limits::default());
});
```

## Step 6: Update CLI (optional)

If the format should be available via the `--format` flag, add a variant to the `Format` enum in `supazip-cli/src/main.rs`:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Zip,
    #[value(name = "7z")]
    SevenZ,
    #[value(name = "mf")]
    MyFormat,
}
```

## Design constraints

- **Object safety.** `ArchiveFormat` must stay object-safe. Do not add generic methods or `Self`-returning methods.
- **No GUI dependencies.** Backends live in `supazip-core` which has no egui/eframe dependency.
- **Resource limits.** Every method must honour the `Limits` parameter. The `max_compression_ratio` guard prevents zip-bomb-class inputs.
- **Cancellation.** Long-running methods must check `progress.is_cancelled()` between entries and short-circuit with `ArchiverError::Cancelled`.
- **Path safety.** Use `safe_join(base, entry)` for every path materialisation during extraction.
