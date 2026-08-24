#![no_main]

//! Fuzz target: plain tar `list` against arbitrary bytes.
//!
//! Looks up the tar backend by extension `"tar"` in the `BACKENDS`
//! registry. The plain-TAR backend is part of WS-B; if it has not been
//! registered yet, the harness returns immediately (no panic) and the
//! target compiles but is effectively a no-op until WS-B lands.

use libfuzzer_sys::fuzz_target;
use supazip_core::{get_backend, Limits};

fuzz_target!(|data: &[u8]| {
    let backend = match get_backend("tar") {
        Some(b) => b,
        None => return,
    };
    let limits = Limits::default();
    let _ = backend.list(Box::new(std::io::Cursor::new(data)), None, &limits);
});
