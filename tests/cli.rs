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

#[test]
fn version_matches_the_published_crate_version() {
    // Reported: docs/reference.md and docs/reference.nl.md said `--version`
    // shows `rum-meter 0.1.0`, which was true for 0.1.0 but went stale when
    // the crate moved to 0.1.1 without the docs being updated. Pin the
    // output to Cargo.toml's version so a future bump cannot drift silently.
    // A debug build also shows the commit `git rev-parse --verify HEAD`
    // prints. A release build stays the crate version. No sha is invented.
    let version = env!("CARGO_PKG_VERSION");
    let expected = if cfg!(debug_assertions) {
        match git_head_sha() {
            Some(sha) => format!("rum-meter {version} ({sha})\n"),
            None => format!("rum-meter {version}\n"),
        }
    } else {
        format!("rum-meter {version}\n")
    };
    for flag in ["--version", "-V"] {
        let output = run(&[flag]);
        assert!(
            output.status.success(),
            "{flag} should exit 0\n{}",
            status_line(&output)
        );
        assert_eq!(output.status.code(), Some(0), "{flag}");
        let stdout = String::from_utf8(output.stdout).expect("version is utf-8");
        assert_eq!(stdout, expected, "{flag}");
    }
}

/// The commit git itself prints. Absent when git does not print one.
fn git_head_sha() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let sha = text.trim();
    if sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(sha.to_string())
    } else {
        None
    }
}
