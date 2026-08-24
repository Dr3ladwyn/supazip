#![no_main]

//! Fuzz target: zip `extract` against arbitrary bytes.
//!
//! Writes the archive to a `tempfile::tempdir()` and lets the backend
//! decompress inside it. Any `ArchiverError` is acceptable; panics are not.
//! The harness exercises the path-traversal guard, the bounded reader, and
//! the `enclosed_name` zip-slip check.

use libfuzzer_sys::fuzz_target;
use supazip_core::{get_backend, Limits, NoOpProgress};

fuzz_target!(|data: &[u8]| {
    let backend = match get_backend("zip") {
        Some(b) => b,
        None => return,
    };
    let Ok(tmp) = tempfile::tempdir() else {
        return;
    };
    let limits = Limits::default();
    let _ = backend.extract(
        Box::new(std::io::Cursor::new(data)),
        tmp.path(),
        &[],
        None,
        &NoOpProgress,
        &limits,
    );
});
