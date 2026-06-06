#![no_main]

//! Fuzz target: tar.xz `create` against arbitrary body bytes.

use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;
use supazip_core::{get_backend, CreateOptions, Limits, NoOpProgress};

#[derive(arbitrary::Arbitrary, Debug)]
pub struct FuzzCreateInput {
    body: Vec<u8>,
}

fuzz_target!(|input: FuzzCreateInput| {
    let backend = match get_backend("tar.xz") {
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
    let out = tmp.path().join("out.tar.xz");
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
