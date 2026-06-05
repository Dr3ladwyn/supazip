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
    let name = if cfg!(windows) {
        "supazip-cli.exe"
    } else {
        "supazip-cli"
    };
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

#[test]
fn list_unknown_extension() {
    let tmp = TempDir::new().expect("tempdir");
    let rar_path = tmp.path().join("archive.rar");
    fs::write(&rar_path, b"not a real rar").expect("write rar");

    let out = Command::new(cli_bin())
        .arg("list")
        .arg(&rar_path)
        .output()
        .expect("run supazip-cli list");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "list of unknown extension should fail; got stdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("error:"),
        "expected `error:` in stderr, got: {stderr}",
    );
}

#[test]
fn list_zip_no_extension() {
    let tmp = TempDir::new().expect("tempdir");
    let no_ext = tmp.path().join("no_extension");
    fs::write(&no_ext, b"some bytes").expect("write");

    let out = Command::new(cli_bin())
        .arg("list")
        .arg(&no_ext)
        .output()
        .expect("run supazip-cli list");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "list of no-extension file should fail; got stdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("error:"),
        "expected `error:` in stderr, got: {stderr}",
    );
}

#[test]
fn extract_zip_with_entry() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out_dir = tmp.path().join("out");
    fs::create_dir_all(&out_dir).expect("mkdir out");

    // Extract only hello.txt; the other entries should NOT appear.
    let out = Command::new(cli_bin())
        .arg("extract")
        .arg(&zip_path)
        .arg("--out")
        .arg(&out_dir)
        .arg("--entry")
        .arg("hello.txt")
        .output()
        .expect("run supazip-cli extract --entry");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "extract --entry failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );

    assert!(
        out_dir.join("hello.txt").is_file(),
        "hello.txt should be extracted"
    );

    // The other entries must not exist on disk.
    for not_expected in ["greet/hi.txt", "data.bin"] {
        assert!(
            !out_dir.join(not_expected).exists(),
            "{not_expected} should NOT be extracted when only hello.txt is requested",
        );
    }
}

#[test]
fn test_zip_corrupted() {
    let tmp = TempDir::new().expect("tempdir");
    let bad = tmp.path().join("corrupt.zip");
    // Random non-zip bytes. The zip crate will reject this when parsing the
    // central directory.
    fs::write(&bad, b"this is not a valid zip file at all\n").expect("write");

    let out = Command::new(cli_bin())
        .arg("test")
        .arg(&bad)
        .output()
        .expect("run supazip-cli test corrupt");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "test of corrupted zip should fail; got stdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("error:"),
        "expected `error:` in stderr, got: {stderr}",
    );
}

#[test]
fn create_7z_then_list() {
    let tmp = TempDir::new().expect("tempdir");
    let src = tmp.path().join("src");
    fs::create_dir(&src).expect("mkdir src");
    let f1 = src.join("one.txt");
    let f2 = src.join("two.txt");
    fs::write(&f1, b"first file\n").expect("write f1");
    fs::write(&f2, b"second file\n").expect("write f2");

    let archive = tmp.path().join("out.7z");
    let out = Command::new(cli_bin())
        .arg("create")
        .arg(&archive)
        .arg(&f1)
        .arg(&f2)
        .output()
        .expect("run supazip-cli create 7z");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "create 7z failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );
    assert!(archive.is_file(), "7z archive should exist on disk");

    let list_out = Command::new(cli_bin())
        .arg("list")
        .arg(&archive)
        .output()
        .expect("run supazip-cli list 7z");

    let list_stdout = String::from_utf8_lossy(&list_out.stdout);
    let list_stderr = String::from_utf8_lossy(&list_out.stderr);
    assert!(
        list_out.status.success(),
        "list 7z failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        list_out.status,
        list_stdout,
        list_stderr,
    );
    for expected in ["one.txt", "two.txt", "2 entries"] {
        assert!(
            list_stdout.contains(expected),
            "list output missing {expected:?}\nfull stdout:\n{list_stdout}\nstderr:\n{list_stderr}",
        );
    }
}

#[test]
fn create_7z_with_password() {
    let tmp = TempDir::new().expect("tempdir");
    let src = tmp.path().join("src");
    fs::create_dir(&src).expect("mkdir src");
    let f1 = src.join("a.txt");
    let f2 = src.join("b.txt");
    fs::write(&f1, b"alpha\n").expect("write a");
    fs::write(&f2, b"bravo\n").expect("write b");

    let archive = tmp.path().join("secret.7z");
    let create_out = Command::new(cli_bin())
        .arg("create")
        .arg(&archive)
        .arg("--password")
        .arg("hunter2")
        .arg(&f1)
        .arg(&f2)
        .output()
        .expect("run supazip-cli create 7z with password");

    let stdout = String::from_utf8_lossy(&create_out.stdout);
    let stderr = String::from_utf8_lossy(&create_out.stderr);
    assert!(
        create_out.status.success(),
        "encrypted 7z create failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        create_out.status,
        stdout,
        stderr,
    );

    // List without password must fail.
    let no_pwd = Command::new(cli_bin())
        .arg("list")
        .arg(&archive)
        .output()
        .expect("run supazip-cli list encrypted 7z without pwd");
    let no_pwd_stderr = String::from_utf8_lossy(&no_pwd.stderr);
    assert!(
        !no_pwd.status.success(),
        "list of encrypted 7z without pwd should fail; got stdout: {}\nstderr: {no_pwd_stderr}",
        String::from_utf8_lossy(&no_pwd.stdout),
    );

    // List with password should succeed and report 2 encrypted entries.
    let with_pwd = Command::new(cli_bin())
        .arg("list")
        .arg(&archive)
        .arg("--password")
        .arg("hunter2")
        .output()
        .expect("run supazip-cli list encrypted 7z with pwd");
    let with_pwd_stdout = String::from_utf8_lossy(&with_pwd.stdout);
    let with_pwd_stderr = String::from_utf8_lossy(&with_pwd.stderr);
    assert!(
        with_pwd.status.success(),
        "list of encrypted 7z with pwd failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        with_pwd.status,
        with_pwd_stdout,
        with_pwd_stderr,
    );
    for expected in ["a.txt", "b.txt", "2 entries", "yes"] {
        assert!(
            with_pwd_stdout.contains(expected),
            "encrypted 7z list output missing {expected:?}\nfull stdout:\n{with_pwd_stdout}",
        );
    }
}

#[test]
fn create_with_format_override() {
    // Create a 7z archive but give the output an arbitrary name (`out.bin`).
    // The `--format 7z` flag must override the extension-based detection.
    // The list side is covered by create_7z_then_list above; this test only
    // proves that `create` honours `--format` for an extension that does not
    // map to a backend.
    let tmp = TempDir::new().expect("tempdir");
    let src = tmp.path().join("src");
    fs::create_dir(&src).expect("mkdir src");
    let f1 = src.join("one.txt");
    fs::write(&f1, b"x\n").expect("write f1");

    let archive = tmp.path().join("out.bin");
    let create_out = Command::new(cli_bin())
        .arg("create")
        .arg(&archive)
        .arg("--format")
        .arg("7z")
        .arg(&f1)
        .output()
        .expect("run supazip-cli create --format 7z");
    let stdout = String::from_utf8_lossy(&create_out.stdout);
    let stderr = String::from_utf8_lossy(&create_out.stderr);
    assert!(
        create_out.status.success(),
        "create --format 7z failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        create_out.status,
        stdout,
        stderr,
    );
    assert!(archive.is_file(), "out.bin should be created");
    // The file should at least look like a 7z archive (magic `7z¼¯'`).
    let head = fs::read(&archive).expect("read out.bin");
    assert!(head.len() >= 6, "file too small to be a 7z archive");
    assert_eq!(
        &head[..6],
        b"7z\xBC\xAF\x27\x1C",
        "out.bin should start with the 7z magic bytes"
    );
}

#[test]
fn test_7z_unencrypted() {
    let tmp = TempDir::new().expect("tempdir");
    let src = tmp.path().join("src");
    fs::create_dir(&src).expect("mkdir src");
    let f1 = src.join("a.txt");
    fs::write(&f1, b"alpha\n").expect("write a");

    let archive = tmp.path().join("plain.7z");
    let create_out = Command::new(cli_bin())
        .arg("create")
        .arg(&archive)
        .arg(&f1)
        .output()
        .expect("run supazip-cli create 7z");
    let stdout = String::from_utf8_lossy(&create_out.stdout);
    let stderr = String::from_utf8_lossy(&create_out.stderr);
    assert!(
        create_out.status.success(),
        "create 7z failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        create_out.status,
        stdout,
        stderr,
    );

    let test_out = Command::new(cli_bin())
        .arg("test")
        .arg(&archive)
        .output()
        .expect("run supazip-cli test 7z");
    let test_stdout = String::from_utf8_lossy(&test_out.stdout);
    let test_stderr = String::from_utf8_lossy(&test_out.stderr);
    assert!(
        test_out.status.success(),
        "test 7z failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        test_out.status,
        test_stdout,
        test_stderr,
    );
    assert!(
        test_stdout.contains("OK:"),
        "expected OK: in stdout, got: {test_stdout}",
    );
}

#[test]
fn test_7z_encrypted() {
    let tmp = TempDir::new().expect("tempdir");
    let src = tmp.path().join("src");
    fs::create_dir(&src).expect("mkdir src");
    let f1 = src.join("a.txt");
    fs::write(&f1, b"alpha\n").expect("write a");

    let archive = tmp.path().join("secret.7z");
    let create_out = Command::new(cli_bin())
        .arg("create")
        .arg(&archive)
        .arg("--password")
        .arg("hunter2")
        .arg(&f1)
        .output()
        .expect("run supazip-cli create 7z with pwd");
    let stdout = String::from_utf8_lossy(&create_out.stdout);
    let stderr = String::from_utf8_lossy(&create_out.stderr);
    assert!(
        create_out.status.success(),
        "encrypted 7z create failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        create_out.status,
        stdout,
        stderr,
    );

    let test_out = Command::new(cli_bin())
        .arg("test")
        .arg(&archive)
        .arg("--password")
        .arg("hunter2")
        .output()
        .expect("run supazip-cli test 7z with pwd");
    let test_stdout = String::from_utf8_lossy(&test_out.stdout);
    let test_stderr = String::from_utf8_lossy(&test_out.stderr);
    assert!(
        test_out.status.success(),
        "test encrypted 7z failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        test_out.status,
        test_stdout,
        test_stderr,
    );
    assert!(
        test_stdout.contains("OK:"),
        "expected OK: in stdout, got: {test_stdout}",
    );
}

#[test]
#[ignore = "enabled in step 7 when encrypted ZIP create is implemented"]
fn create_zip_with_password() {
    // Will be re-enabled in step 7 (feat(core): preserve error source chain;
    // support encrypted ZIP create). Until then, encrypted ZIP create drops
    // the password (`let _pwd = password;` in zip.rs) and produces an
    // unencrypted archive; this test will then assert that the produced zip
    // is genuinely encrypted.
    let tmp = TempDir::new().expect("tempdir");
    let src = tmp.path().join("src");
    fs::create_dir(&src).expect("mkdir src");
    let f1 = src.join("a.txt");
    fs::write(&f1, b"alpha\n").expect("write a");

    let archive = tmp.path().join("secret.zip");
    let create_out = Command::new(cli_bin())
        .arg("create")
        .arg(&archive)
        .arg("--password")
        .arg("hunter2")
        .arg(&f1)
        .output()
        .expect("run supazip-cli create zip with pwd");
    let stdout = String::from_utf8_lossy(&create_out.stdout);
    let stderr = String::from_utf8_lossy(&create_out.stderr);
    assert!(
        create_out.status.success(),
        "encrypted zip create failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        create_out.status,
        stdout,
        stderr,
    );

    let list_out = Command::new(cli_bin())
        .arg("list")
        .arg(&archive)
        .output()
        .expect("run supazip-cli list encrypted zip");
    let list_stdout = String::from_utf8_lossy(&list_out.stdout);
    let list_stderr = String::from_utf8_lossy(&list_out.stderr);
    assert!(
        list_out.status.success(),
        "list encrypted zip failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        list_out.status,
        list_stdout,
        list_stderr,
    );
    assert!(
        list_stdout.contains("yes"),
        "encrypted zip should report CRYPT=yes for entries; got: {list_stdout}",
    );
}
