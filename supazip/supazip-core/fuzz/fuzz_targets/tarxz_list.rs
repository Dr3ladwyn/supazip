#![no_main]

//! Fuzz target: tar.xz `list` against arbitrary bytes.

use libfuzzer_sys::fuzz_target;
use supazip_core::{get_backend, Limits};

fuzz_target!(|data: &[u8]| {
    let backend = match get_backend("tar.xz") {
        Some(b) => b,
        None => return,
    };
    let limits = Limits::default();
    let _ = backend.list(Box::new(std::io::Cursor::new(data.to_vec())), None, &limits);
});
