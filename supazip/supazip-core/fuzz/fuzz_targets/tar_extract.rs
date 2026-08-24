#![no_main]

//! Fuzz target: plain tar `extract` against arbitrary bytes.

use libfuzzer_sys::fuzz_target;
use supazip_core::{get_backend, Limits, NoOpProgress};

fuzz_target!(|data: &[u8]| {
    let backend = match get_backend("tar") {
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
