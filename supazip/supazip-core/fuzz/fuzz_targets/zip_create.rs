#![no_main]

//! Fuzz target: zip `create` against arbitrary body bytes.
//!
//! Writes the fuzz body to a temp file and asks the zip backend to archive
//! it. The body length is controlled by the fuzzer, which exercises the
//! `start_file` path, the per-entry streaming write, and the AES-256 /
//! deflate method negotiation through `CreateOptions`.

use libfuzzer_sys::fuzz_target;
use supazip_core::{get_backend, CreateOptions, Limits, NoOpProgress};
use std::path::PathBuf;

#[derive(arbitrary::Arbitrary, Debug)]
pub struct FuzzCreateInput {
    body: Vec<u8>,
}

fuzz_target!(|input: FuzzCreateInput| {
    let backend = match get_backend("zip") {
        Some(b) => b,
        None => return,
    };
    let Ok(tmp) = tempfile::tempdir() else {
        return;
    };
    let entry_path: PathBuf = tmp.path().join("entry.bin");
    if std::fs::write(&entry_path, &input.body).is_err() {
        return;
    }
    let out = tmp.path().join("out.zip");
    let Ok(f) = std::fs::File::create(&out) else {
        return;
    };
    let limits = Limits::default();
    let opts = CreateOptions::default();
    let _ = backend.create(
        Box::new(f),
        &[entry_path],
        &opts,
        None,
        &NoOpProgress,
        &limits,
    );
});
