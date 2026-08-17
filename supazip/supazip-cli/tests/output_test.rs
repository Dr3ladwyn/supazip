//! Integration tests for `--output json|yaml|text` on `list` and `test` commands.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

/// Locate the freshly-built `supazip-cli` binary next to the test binary.
fn cli_bin() -> PathBuf {
    if let Some(p) = option_env!("CARGO_BIN_EXE_supazip-cli") {
        return PathBuf::from(p);
    }
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

    for (name, body) in [
        ("hello.txt", b"hello supazip\n" as &[u8]),
        ("greet/hi.txt", b"hi from a folder entry\n"),
        ("data.bin", &[0xAA, 0xBB, 0xCC, 0xDD]),
    ] {
        zw.start_file(name, options).expect("start_file");
        zw.write_all(body).expect("write_all body");
    }
    zw.finish().expect("finish");
}

// ---------------------------------------------------------------------------
// list --output json
// ---------------------------------------------------------------------------

#[test]
fn list_json_output() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out = Command::new(cli_bin())
        .args(["list", "--output", "json"])
        .arg(&zip_path)
        .output()
        .expect("run supazip-cli list --output json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "list --output json failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );

    // Must be valid JSON (an array of objects).
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout is not valid JSON");
    let arr = parsed.as_array().expect("expected JSON array");
    assert_eq!(arr.len(), 3, "expected 3 entries in JSON output");

    // Spot-check fields.
    let names: Vec<&str> = arr.iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"hello.txt"), "missing hello.txt: {names:?}");
    assert!(
        names.contains(&"greet/hi.txt"),
        "missing greet/hi.txt: {names:?}"
    );
    assert!(names.contains(&"data.bin"), "missing data.bin: {names:?}");
}

// ---------------------------------------------------------------------------
// list --output yaml
// ---------------------------------------------------------------------------

#[test]
fn list_yaml_output() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out = Command::new(cli_bin())
        .args(["list", "--output", "yaml"])
        .arg(&zip_path)
        .output()
        .expect("run supazip-cli list --output yaml");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "list --output yaml failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );

    // YAML output should contain entry names as keys.
    assert!(stdout.contains("hello.txt"), "missing hello.txt in YAML");
    assert!(
        stdout.contains("greet/hi.txt"),
        "missing greet/hi.txt in YAML"
    );
    assert!(stdout.contains("data.bin"), "missing data.bin in YAML");
    // Should look like a YAML list (starts with `- `).
    assert!(
        stdout.trim_start().starts_with('-'),
        "YAML output should start with '-': {}",
        stdout.lines().next().unwrap_or("")
    );
}

// ---------------------------------------------------------------------------
// test --output json
// ---------------------------------------------------------------------------

#[test]
fn test_json_output() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out = Command::new(cli_bin())
        .args(["test", "--output", "json"])
        .arg(&zip_path)
        .output()
        .expect("run supazip-cli test --output json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "test --output json failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout is not valid JSON");
    assert_eq!(
        parsed["ok"], true,
        "expected ok=true in test JSON output, got: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// test --output yaml
// ---------------------------------------------------------------------------

#[test]
fn test_yaml_output() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out = Command::new(cli_bin())
        .args(["test", "--output", "yaml"])
        .arg(&zip_path)
        .output()
        .expect("run supazip-cli test --output yaml");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "test --output yaml failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );

    assert!(
        stdout.contains("ok: true"),
        "expected 'ok: true' in YAML output, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Default output is text (backward compat)
// ---------------------------------------------------------------------------

#[test]
fn default_output_is_text() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out = Command::new(cli_bin())
        .arg("list")
        .arg(&zip_path)
        .output()
        .expect("run supazip-cli list (default output)");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "default list failed\nstderr: {}",
        String::from_utf8_lossy(&out.stderr),
    );

    // The default text output uses a fixed-width table header.
    assert!(
        stdout.contains("IDX"),
        "text output should contain IDX header, got: {stdout}"
    );
    assert!(
        stdout.contains("entries"),
        "text output should contain 'entries', got: {stdout}"
    );
    // Must NOT be JSON (no leading '[').
    assert!(
        !stdout.trim_start().starts_with('['),
        "default output should not be JSON: {}",
        &stdout[..stdout.len().min(200)]
    );
}

#[test]
fn list_text_output_contains_header_columns() {
    let tmp = TempDir::new().expect("tempdir");
    let zip_path = tmp.path().join("sample.zip");
    build_small_zip(&zip_path);

    let out = Command::new(cli_bin())
        .arg("list")
        .arg(&zip_path)
        .output()
        .expect("run supazip-cli list text");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "list text failed\nstatus: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        stdout,
        stderr,
    );

    for col in ["IDX", "METHOD", "SIZE", "COMPRESSED", "CRYPT", "NAME"] {
        assert!(
            stdout.contains(col),
            "text list should contain header column {col:?}, got: {stdout}"
        );
    }
    assert!(
        stdout.contains("─"),
        "text list should use the U+2500 rule from cli-table spec, got: {stdout}"
    );
    // Piped stdout must stay uncoloured (Command::output captures a pipe).
    assert!(
        !stdout.contains('\u{1b}'),
        "piped text list must not contain ANSI color, got: {stdout:?}"
    );
    // Must NOT be JSON (no leading '[').
    assert!(
        !stdout.trim_start().starts_with('['),
        "default output should not be JSON: {}",
        &stdout[..stdout.len().min(200)]
    );
}
