#![no_main]

//! Fuzz target: tar.gz `list` against arbitrary bytes.
//!
//! Looks up the backend by the composite extension `"tar.gz"`. If the
//! `TarGzBackend` has not been registered yet (WS-B is in flight), the
//! harness returns immediately.

use libfuzzer_sys::fuzz_target;
use supazip_core::{get_backend, Limits};

fuzz_target!(|data: &[u8]| {
    let backend = match get_backend("tar.gz") {
        Some(b) => b,
        None => return,
    };
    let limits = Limits::default();
    let _ = backend.list(Box::new(data), None, &limits);
});
