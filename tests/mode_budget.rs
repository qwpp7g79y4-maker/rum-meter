//! `mode` sits once on the resource, same as module, type and name, and is
//! copied into the identity of every instance. A long shared `mode` is
//! refused before that copy, the same as a long module.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .output()
        .expect("rum-meter binary")
}

fn instances(n: u64) -> String {
    (0..n)
        .map(|i| format!(r#"{{"index_key":{i}}}"#))
        .collect::<Vec<_>>()
        .join(",")
}

fn write_state(dir: &std::path::Path, name: &str, mode: &str) -> String {
    let path = dir.join(name);
    let body = format!(
        r#"{{"version":4,"outputs":{{"note":{{"value":"CANARY-SECRET-MODE"}}}},"resources":[{{"mode":"{mode}","type":"t","name":"n","instances":[{}]}}]}}"#,
        instances(2500)
    );
    std::fs::write(&path, body).expect("write state");
    path.into_os_string().into_string().expect("utf-8 path")
}

fn assert_refused(output: &Output, path: &str, elapsed: Duration) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "a shared label over the budget should exit 2"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty, len {}",
        output.stdout.len()
    );
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {path}: resource label is repeated across too many instances\n")
    );
    assert!(!stderr.contains("CANARY-SECRET-MODE"));
    assert!(!stderr.contains(&"a".repeat(64)));
    assert!(
        elapsed < Duration::from_secs(1),
        "refusal took {elapsed:?}, the copy should not run"
    );
}

#[test]
fn long_shared_mode_exits_2_and_short_2500_still_counts() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mode-budget");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let long = write_state(&dir, "long-mode.json", &"a".repeat(80_000));
    let short = write_state(&dir, "short-mode.json", "data");

    let start = Instant::now();
    let refused = run(&["count", &long]);
    assert_refused(&refused, &long, start.elapsed());

    let counted = run(&["count", &short]);
    assert_eq!(counted.status.code(), Some(0));
    let stdout = String::from_utf8(counted.stdout).expect("stdout is utf-8");
    assert!(stdout.contains("rum: 0\n"), "{stdout}");
    assert!(counted.stderr.is_empty());

    let start = Instant::now();
    let old_long = run(&["diff", &long, &short]);
    assert_refused(&old_long, &long, start.elapsed());

    let start = Instant::now();
    let new_long = run(&["diff", &short, &long]);
    assert_refused(&new_long, &long, start.elapsed());
}
