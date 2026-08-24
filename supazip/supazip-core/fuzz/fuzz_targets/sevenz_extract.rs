#![no_main]

//! Fuzz target: 7z `extract` against arbitrary bytes.
//!
//! Exercises the 7z `for_each_entries` loop, the `safe_join` path-traversal
//! guard, and the AES probe at the start of the extract path. This is one
//! of the two representative targets the CI smoke job runs (the other is
//! `zip_list`); together they cover the two backends that existed before
//! 0.2.0 and stress both metadata and write paths.

use libfuzzer_sys::fuzz_target;
use supazip_core::{get_backend, Limits, NoOpProgress};

fuzz_target!(|data: &[u8]| {
    let backend = match get_backend("7z") {
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
