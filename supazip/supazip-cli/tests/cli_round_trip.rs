//! End-to-end test for the SupaZip CLI.
//!
//! Spins up a small ZIP archive in a tempdir using the `zip` crate directly,
//! then drives the compiled `supazip-cli` binary to list and extract it, and
//! asserts on the output. We use `std::process::Command` rather than
//! `assert_cmd` so the test has no extra dependencies and runs the same way
//! `cargo run` would.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

/// Locate the freshly-built `supazip-cli` binary next to the test binary.
fn cli_bin() -> PathBuf {
    // `CARGO_BIN_EXE_supazip-cli` is set by cargo when an integration test in
    // a sibling crate builds the binary. Fall back to a path discovery in case
    // it's not (older toolchains).
    if let Some(p) = option_env!("CARGO_BIN_EXE_supazip-cli") {
        return PathBuf::from(p);
    }
    // Fallback: ../target/debug/supazip-cli(.exe) relative to the test binary.
    let exe = std::env::current_exe().expect("current_exe");
    let mut p = exe.parent().expect("parent").to_path_buf();
    p.pop();
    p.push("debug");
    let name = if cfg!(windows) { "supazip-cli.exe" } else { "supazip-cli" };
    p.push(name);
    p
}

fn build_small_zip(zip_path: &PathBuf) {
    let file = fs::File::create(zip_path).expect("create zip");
    let mut zw = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut contents: Vec<(&str, &[u8])> = vec![
        ("hello.txt", b"hello supazip\n" as &[u8]),
        ("greet/hi.txt", b"hi from a folder entry\n"),
        ("data.bin", &[0xAA, 0xBB, 0xCC, 0xDD]),
    ];

    for (name, body) in contents.drain(..) {
        zw.start_file(name, options).expect("start_file");
        zw.write_all(body).expect("write_all body");
    }
    zw.finish().expect("finish");
}

#[test]
fn list_zip_via_cli() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out = Command::new(cli_bin())
        .arg("list")
        .arg(&zip_path)
        .output()
        .expect("run supazip-cli list");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "list failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );

    // The CLI prints the entry name on a fixed-width line. We assert
    // substrings rather than the full table to keep the test robust against
    // formatting tweaks.
    for expected in ["hello.txt", "greet/hi.txt", "data.bin", "3 entries"] {
        assert!(
            stdout.contains(expected),
            "stdout did not contain {expected:?}\nfull stdout:\n{stdout}\nstderr:\n{stderr}",
        );
    }
}

#[test]
fn extract_zip_via_cli_round_trip() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out_dir = tmp.path().join("out");
    fs::create_dir_all(&out_dir).expect("mkdir out");

    let out = Command::new(cli_bin())
        .arg("extract")
        .arg(&zip_path)
        .arg("--out")
        .arg(&out_dir)
        .output()
        .expect("run supazip-cli extract");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "extract failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );

    // The CLI's extract currently uses `enclosed_name` and writes into the
    // *current working directory*, honouring `--out` by chdir-ing for the
    // duration of the call. So the files should land directly under out_dir.
    let hello = fs::read(out_dir.join("hello.txt")).expect("read hello.txt");
    assert_eq!(hello, b"hello supazip\n");

    let hi = fs::read(out_dir.join("greet/hi.txt")).expect("read greet/hi.txt");
    assert_eq!(hi, b"hi from a folder entry\n");

    let data = fs::read(out_dir.join("data.bin")).expect("read data.bin");
    assert_eq!(data, vec![0xAA, 0xBB, 0xCC, 0xDD]);
}

#[test]
fn test_zip_via_cli_reports_ok() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out = Command::new(cli_bin())
        .arg("test")
        .arg(&zip_path)
        .output()
        .expect("run supazip-cli test");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "test failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );
    assert!(
        stdout.contains("OK:"),
        "expected `OK:` in stdout, got: {stdout}\nstderr: {stderr}",
    );
}
