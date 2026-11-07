//! `count` can load several state files in one invocation, and classifies
//! them together. The label budget from `tests/label_budget.rs` must apply
//! to the sum of every file's product in that invocation, not to each file
//! checked alone: several files that are each under the budget must not be
//! able to add up to more than the budget once combined. A path repeated in
//! the same invocation must be refused before it is read at all, so it
//! cannot be counted (and copied) more than once either.

use std::path::PathBuf;
use std::process::{Command, Output};

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

/// `mode` (7) + `type` (1) + `name` (1) + `module` (`module_len`) is the
/// label `classify` copies once per instance. `name` stays one character so
/// it does not shift the arithmetic, and differs between files so their
/// addresses do not collide.
fn write_state(
    dir: &std::path::Path,
    file_name: &str,
    resource_name: &str,
    module_len: usize,
) -> String {
    let path = dir.join(file_name);
    let body = format!(
        r#"{{"version":4,"resources":[{{"mode":"managed","type":"t","name":"{resource_name}","module":"{}","instances":[{}]}}]}}"#,
        "a".repeat(module_len),
        instances(32768)
    );
    std::fs::write(&path, body).expect("write state");
    path.into_os_string().into_string().expect("utf-8 path")
}

#[test]
fn two_files_each_under_budget_at_exact_combined_limit_is_accepted() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("count-multi-file-budget-exact");
    std::fs::create_dir_all(&dir).expect("temp dir");
    // Each file's product is 512 * 32768 = 16_777_216 (16 MiB), well under
    // the 32 MiB budget alone. Combined, 33_554_432 == MAX exactly.
    let a = write_state(&dir, "a.json", "a", 503);
    let b = write_state(&dir, "b.json", "b", 503);

    let counted = run(&["count", &a, &b, "--json"]);
    assert_eq!(
        counted.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&counted.stderr)
    );
}

#[test]
fn two_files_each_under_budget_but_over_combined_is_refused() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("count-multi-file-budget-over");
    std::fs::create_dir_all(&dir).expect("temp dir");
    // File a: 512 * 32768 = 16_777_216 (16 MiB), under budget alone.
    // File b: 513 * 32768 = 16_809_984 (just over 16 MiB), under budget alone.
    // Combined: 33_587_200, 32_768 bytes over the 32 MiB budget.
    let a = write_state(&dir, "a.json", "a", 503);
    let b = write_state(&dir, "b.json", "b", 504);

    let counted = run(&["count", &a, &b, "--json"]);
    assert_eq!(
        counted.status.code(),
        Some(2),
        "two files each under budget alone must still be refused once their sum is over it"
    );
    assert!(counted.stdout.is_empty());
    let stderr = String::from_utf8(counted.stderr).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!(
            "rum-meter: {b}: resource label is repeated across too many instances; split the state into smaller files\n"
        )
    );
}

#[test]
fn eight_files_each_under_budget_but_over_combined_is_refused() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("count-multi-file-budget-eight");
    std::fs::create_dir_all(&dir).expect("temp dir");
    // Eight files at 99.6% of the budget each (16_744_448 of 16_777_216 per
    // file alone would be under budget many times over); here each file is
    // just under the *whole* 32 MiB budget alone, and the eighth pushes the
    // running sum over it.
    let names = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let files: Vec<String> = names
        .iter()
        .map(|name| write_state(&dir, &format!("{name}.json"), name, 503))
        .collect();
    let args: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut full_args = vec!["count"];
    full_args.extend(args.iter());
    full_args.push("--json");

    let counted = run(&full_args);
    assert_eq!(
        counted.status.code(),
        Some(2),
        "the running sum across all eight files must be checked, not each file alone"
    );
    assert!(counted.stdout.is_empty());
}

#[test]
fn repeated_path_is_refused_before_any_file_is_read() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("count-repeated-path");
    std::fs::create_dir_all(&dir).expect("temp dir");
    // The path does not exist. If the fix rejects the repeat before
    // opening any file, the error is the repeat message, never a load
    // error like "unreadable".
    let missing = dir
        .join("missing.json")
        .into_os_string()
        .into_string()
        .expect("utf-8 path");

    let counted = run(&["count", &missing, &missing, "--json"]);
    assert_eq!(counted.status.code(), Some(2));
    assert!(counted.stdout.is_empty());
    let stderr = String::from_utf8(counted.stderr).expect("stderr is utf-8");
    assert!(
        !stderr.contains("unreadable"),
        "the repeat must be caught before the file is opened, got: {stderr}"
    );
    assert!(stderr.contains(&missing), "{stderr}");
}

#[test]
fn repeated_path_thirty_two_times_is_refused_before_classify() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("count-repeated-path-many");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = write_state(&dir, "one.json", "a", 1);
    let args: Vec<&str> = std::iter::once("count")
        .chain(std::iter::repeat_n(path.as_str(), 32))
        .chain(std::iter::once("--json"))
        .collect();

    let counted = run(&args);
    assert_eq!(counted.status.code(), Some(2));
    assert!(counted.stdout.is_empty());
}
