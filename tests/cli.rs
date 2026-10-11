use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .output()
        .expect("rum-meter binary")
}

fn status_line(output: &Output) -> String {
    format!(
        "status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn help_shows_every_option() {
    let output = run(&["--help"]);
    assert!(
        output.status.success(),
        "help should exit 0\n{}",
        status_line(&output)
    );
    let help = String::from_utf8(output.stdout).expect("help is utf-8");
    for option in [
        "--json",
        "--explain",
        "--top",
        "--prices",
        "--hours-per-month",
        "--max-rum",
        "--max-increase",
        "count",
        "diff",
    ] {
        assert!(help.contains(option), "missing {option} in:\n{help}");
    }
}

#[test]
fn unknown_option_exits_3() {
    let output = run(&["--not-an-option"]);
    assert_eq!(
        output.status.code(),
        Some(3),
        "unknown option should exit 3\n{}",
        status_line(&output)
    );
}

#[test]
fn count_without_state_exits_3() {
    let output = run(&["count"]);
    assert_eq!(
        output.status.code(),
        Some(3),
        "count without a state file should exit 3\n{}",
        status_line(&output)
    );
}

#[test]
fn diff_with_one_file_exits_3() {
    let output = run(&["diff", "only.json"]);
    assert_eq!(
        output.status.code(),
        Some(3),
        "diff with one file should exit 3\n{}",
        status_line(&output)
    );
}
