use std::fs;
use std::process::Command;

#[test]
fn man_generates_file() {
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path();

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "supazip-cli",
            "--ignore-rust-version",
            "--",
            "man",
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("failed to execute supazip-cli man");
    assert!(
        output.status.success(),
        "supazip man failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let man_file = out_dir.join("supazip.1");
    assert!(man_file.exists(), "supazip.1 should be created");
    let meta = fs::metadata(&man_file).unwrap();
    assert!(
        meta.len() > 100,
        "man page should be >100 bytes, got {}",
        meta.len()
    );
}
