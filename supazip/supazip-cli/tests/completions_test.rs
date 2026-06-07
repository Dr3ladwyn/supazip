use std::process::Command;

fn supazip_bin() -> String {
    // When running under `cargo test`, the binary is built by the test harness.
    // We use `cargo run` to ensure the latest code is compiled and executed.
    String::from("cargo")
}

fn run_completions(shell: &str) -> Vec<u8> {
    let output = Command::new(supazip_bin())
        .args(["run", "-p", "supazip-cli", "--", "completions", shell])
        .output()
        .expect("failed to execute supazip completions");
    assert!(
        output.status.success(),
        "supazip completions {shell} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn bash_completions_have_content() {
    let stdout = run_completions("bash");
    assert!(
        stdout.len() > 100,
        "bash completions too short ({} bytes)",
        stdout.len()
    );
}

#[test]
fn fish_completions_contain_complete() {
    let stdout = run_completions("fish");
    let text = String::from_utf8_lossy(&stdout);
    assert!(
        text.contains("complete -c supazip"),
        "fish completions missing 'complete -c supazip'"
    );
}
