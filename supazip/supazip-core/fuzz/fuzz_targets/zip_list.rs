#![no_main]

//! Fuzz target: zip `list` against arbitrary bytes.
//!
//! Feeds the entire fuzz input to `ZipBackend::list` through the
//! `supazip_core::formats::BACKENDS` registry. The backend is expected to
//! return `Ok(Vec<ArchiveEntry>)` on a valid zip and an `ArchiverError` on
//! anything else. The harness must not panic, must not `unwrap`, and must
//! not allocate unboundedly: the `Limits::default()` ceiling on archive size
//! guards the latter.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let backend = match supazip_core::get_backend("zip") {
        Some(b) => b,
        None => return, // registry not initialised; nothing to fuzz yet
    };
    let limits = supazip_core::Limits::default();
    let cursor = std::io::Cursor::new(data);
    let _ = backend.list(Box::new(cursor), None, &limits);
});
