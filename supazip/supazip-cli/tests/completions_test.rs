use std::path::PathBuf;
use std::process::Command;

fn supazip_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_supazip-cli") {
        return PathBuf::from(path);
    }
    let mut path = std::env::current_exe().expect("current_exe");
    path.pop();
    path.pop();
    path.push(if cfg!(windows) {
        "supazip-cli.exe"
    } else {
        "supazip-cli"
    });
    path
}

fn run_completions(shell: &str) -> Vec<u8> {
    let output = Command::new(supazip_bin())
        .args(["completions", shell])
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
