#![no_main]

//! Fuzz target: 7z `list` against arbitrary bytes.
//!
//! The 7z backend buffers the input (it requires `Read + Seek` for the
//! central directory header), so a large fuzz input could pressure memory.
//! `Limits::default()` caps the buffer at 4 GiB and the fuzzer's default
//! input length is well below that in practice.

use libfuzzer_sys::fuzz_target;
use supazip_core::{get_backend, Limits};

fuzz_target!(|data: &[u8]| {
    let backend = match get_backend("7z") {
        Some(b) => b,
        None => return,
    };
    let limits = Limits::default();
    let _ = backend.list(Box::new(std::io::Cursor::new(data.to_vec())), None, &limits);
});
