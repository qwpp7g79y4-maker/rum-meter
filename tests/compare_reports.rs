//! `--compare FILE` prints the difference between a saved JSON report and this run.
//!
//! There is one flag. Module paths, type names and addresses are matched by
//! their text. A path that stays is not given a new number. The difference
//! is not a threshold: exit 1 stays `--max-rum` and `--max-increase`.

use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;

const COUNT_REPORT: &str = "tests/fixtures/compare-count.json";
const DIFF_REPORT: &str = "tests/fixtures/compare-diff.json";
const ONE: &str = "tests/fixtures/v4_one_managed.json";
const TOP: &str = "tests/fixtures/v4_top.json";
const PRICES: &str = "tests/fixtures/example-prices.json";
const ESTIMATE: &str = "estimate: HCP Terraform bills the hourly peak; this is a single snapshot\n";

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd
}

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let output = bin().args(args).output().expect("rum-meter binary");
    (
        output.status.code(),
        String::from_utf8(output.stdout).expect("stdout is utf-8"),
        String::from_utf8(output.stderr).expect("stderr is utf-8"),
    )
}

fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "compare-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write");
    path.into_os_string().into_string().expect("utf-8 path")
}

#[test]
fn help_has_one_compare_flag() {
    let (code, help, _) = run(&["--help"]);
    assert_eq!(code, Some(0));
    let mut options = false;
    let mut names = Vec::new();
    for line in help.lines() {
        if !options {
            if line == "Options:" {
                options = true;
            }
            continue;
        }
        if line.is_empty() {
            break;
        }
        for word in line.split_whitespace() {
            if let Some(rest) = word.strip_prefix("--") {
                if rest.chars().all(|c| c.is_ascii_lowercase() || c == '-') && !rest.is_empty() {
                    names.push(format!("--{rest}"));
                }
            }
        }
    }
    assert_eq!(
        names.iter().filter(|name| *name == "--compare").count(),
        1,
        "{names:?}"
    );
    assert!(!names.iter().any(|name| name == "--cmp"));
    assert!(!names.iter().any(|name| name == "--diff-report"));
    assert!(!names.iter().any(|name| name == "--report-diff"));
}

#[test]
fn a_count_report_shows_the_difference_and_keeps_paths() {
    let (code, out, err) = run(&["count", "--compare", COUNT_REPORT, ONE]);
    assert_eq!(code, Some(0), "{err}");
    assert!(err.is_empty(), "{err}");
    assert_eq!(
        out,
        format!(
            "\
compared: {COUNT_REPORT}
total rum: 0 -> 1
rum delta: +1
modules:
modules added:
  (root): 1
modules removed:
types:
types added:
  random_id: 1
types removed:
{ESTIMATE}"
        )
    );
    assert!(!out.contains("(root)["));
    assert!(!out.contains("random_id["));
    assert!(!out.contains("CANARY"));
}

#[test]
fn reordered_rows_are_not_renumbered() {
    let (code, json, err) = run(&["count", "--json", "--prices", PRICES, TOP]);
    assert_eq!(code, Some(0), "{err}");
    let mut report: Value = serde_json::from_str(&json).expect("count json");
    let modules = report["by_module"].as_array().expect("modules").clone();
    assert!(modules.len() >= 2, "need two modules to reverse");
    report["by_module"]
        .as_array_mut()
        .expect("modules")
        .reverse();
    report["by_type"].as_array_mut().expect("types").reverse();
    report["costs"].as_array_mut().expect("costs").reverse();
    let dir = scratch();
    let saved = write(&dir, "saved.json", report.to_string().as_bytes());
    let (code, out, err) = run(&["count", "--prices", PRICES, "--compare", &saved, TOP]);
    assert_eq!(code, Some(0), "{err}");
    let total = report["total_rum"].as_u64().expect("total");
    assert_eq!(
        out,
        format!(
            "\
compared: {saved}
total rum: {total} -> {total}
rum delta: 0
modules:
modules added:
modules removed:
types:
types added:
types removed:
costs:
costs added:
costs removed:
{ESTIMATE}"
        ),
        "a different array order must not add or renumber a path"
    );
    assert!(!out.contains("example-a"));
    assert!(!out.contains("(other"));

    let name = report["by_module"][0]["name"]
        .as_str()
        .expect("name")
        .to_string();
    let count = report["by_module"][0]["count"].as_u64().expect("count");
    report["by_module"][0]["count"] = Value::from(count + 1);
    report["total_rum"] = Value::from(total + 1);
    let changed = write(&dir, "changed.json", report.to_string().as_bytes());
    let (code, out, err) = run(&["count", "--prices", PRICES, "--compare", &changed, TOP]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains(&format!("  {name}: {} -> {count}\n", count + 1)),
        "{out}"
    );
    assert!(!out.contains(&format!("{name}[")), "{out}");
    assert!(!out.contains("(other"), "{out}");
    assert!(
        !out.contains(&format!("modules removed:\n  {name}:")),
        "{out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_address_keeps_its_index_when_the_list_order_changes() {
    let dir = scratch();
    let old = write(
        &dir,
        "old.json",
        br#"{
  "version": 4,
  "resources": [
    {
      "mode": "managed",
      "type": "random_id",
      "name": "kept",
      "instances": [{ "index_key": 0 }]
    },
    {
      "mode": "managed",
      "type": "random_id",
      "name": "zzz",
      "instances": [{ "index_key": 0 }]
    }
  ]
}"#,
    );
    let new = write(
        &dir,
        "new.json",
        br#"{
  "version": 4,
  "resources": [
    {
      "mode": "managed",
      "type": "random_id",
      "name": "kept",
      "instances": [{ "index_key": 1 }]
    },
    {
      "mode": "managed",
      "type": "random_id",
      "name": "extra",
      "instances": [{ "index_key": 0 }]
    }
  ]
}"#,
    );
    let saved = write(
        &dir,
        "saved.json",
        br#"{
  "schema_version": 1,
  "tool": "rum-meter",
  "estimate": "HCP Terraform bills the hourly peak; this is a single snapshot",
  "inputs": { "old": "old", "new": "new" },
  "total_rum": { "old": 2, "new": 2 },
  "by_module": [{ "name": "(root)", "count": 2 }],
  "by_type": [{ "name": "random_id", "count": 2 }],
  "added": ["random_id.kept[1]", "random_id.zzz[0]"],
  "removed": ["random_id.zzz[0]", "random_id.kept[0]"],
  "changed_rule": [],
  "rum_delta": 0
}"#,
    );
    let (code, out, err) = run(&["diff", "--compare", &saved, &old, &new]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("  + random_id.extra[0]\n"),
        "live-only address\n{out}"
    );
    assert!(
        out.contains("  - random_id.zzz[0]\n"),
        "saved-only address\n{out}"
    );
    assert!(
        !out.contains("random_id.kept[1]"),
        "an address in both lists is not renumbered\n{out}"
    );
    assert!(
        !out.contains("random_id.kept[0]"),
        "a shared removed address is not renumbered\n{out}"
    );
    assert!(!out.contains("random_id.kept[2]"), "{out}");
    assert!(out.contains("rum delta: 0 -> 0\n"), "{out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stdin_stays_the_state_on_either_side_of_compare() {
    for args in [
        ["count", "--compare", COUNT_REPORT, "-"],
        ["count", "-", "--compare", COUNT_REPORT],
    ] {
        let state = File::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ONE)).expect("state");
        let output = bin()
            .args(args)
            .stdin(Stdio::from(state))
            .output()
            .expect("rum-meter binary");
        let err = String::from_utf8(output.stderr).expect("stderr");
        let out = String::from_utf8(output.stdout).expect("stdout");
        assert_eq!(output.status.code(), Some(0), "{args:?} {err}");
        assert!(out.contains("total rum: 0 -> 1\n"), "{args:?}\n{out}");
        assert!(
            out.contains(&format!("compared: {COUNT_REPORT}\n")),
            "{out}"
        );
    }
}

#[test]
fn the_difference_is_not_a_new_threshold() {
    let (code, out, err) = run(&["count", "--max-rum", "0", "--compare", COUNT_REPORT, ONE]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.is_empty(), "{err}");
    assert!(out.contains("total rum: 0 -> 1\n"), "{out}");

    let (code, out, err) = run(&[
        "diff",
        "--max-increase",
        "0",
        "--compare",
        DIFF_REPORT,
        "tests/fixtures/v4_diff_new.json",
        "tests/fixtures/v4_diff_old.json",
    ]);
    assert_eq!(code, Some(1), "{err}");
    assert!(out.contains("rum delta:"), "{out}");
    assert!(!out.is_empty());
}

#[test]
fn a_missing_report_is_exit_2_and_a_missing_output_directory_stays_exit_2() {
    let (code, out, err) = run(&[
        "count",
        "--compare",
        "tests/fixtures/missing-compare.json",
        ONE,
    ]);
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty(), "{out}");
    assert_eq!(
        err,
        "rum-meter: tests/fixtures/missing-compare.json: unreadable (not found); check that the path exists and that you may read it\n"
    );

    let (code, out, err) = run(&["count", "--compare", "tests/fixtures", ONE]);
    assert_eq!(code, Some(2));
    assert!(out.is_empty(), "{out}");
    assert_eq!(
        err,
        "rum-meter: tests/fixtures: not a regular file; pass a regular file\n"
    );

    let (code, out, err) = run(&[
        "count",
        "--compare",
        COUNT_REPORT,
        "--output",
        "tests/no-such-output-dir/report.txt",
        ONE,
    ]);
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty(), "{out}");
    assert_eq!(
        err,
        "rum-meter: cannot write report to tests/no-such-output-dir/report.txt; check that the parent directory exists and is writable, the tool does not create it\n"
    );
    assert!(
        !PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/no-such-output-dir")
            .exists()
    );
}

#[test]
fn a_report_over_128_mib_is_exit_2() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("compare-cap");
    fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("sparse-compare.json");
    let file = File::create(&path).expect("create");
    file.set_len(129 * 1024 * 1024).expect("set_len");
    drop(file);
    let path = path.into_os_string().into_string().expect("utf-8 path");
    let (code, out, err) = run(&["count", "--compare", &path, ONE]);
    let _ = fs::remove_file(&path);
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty(), "{out}");
    assert_eq!(
        err,
        format!("rum-meter: {path}: file is larger than 128 MiB; pass a smaller file\n")
    );
}

#[test]
fn the_wrong_kind_of_report_is_exit_2() {
    let (code, out, err) = run(&["count", "--compare", DIFF_REPORT, ONE]);
    assert_eq!(code, Some(2));
    assert!(out.is_empty(), "{out}");
    assert_eq!(
        err,
        format!(
            "rum-meter: {DIFF_REPORT}: report is a diff report; pass a count report written with --json\n"
        )
    );

    let (code, out, err) = run(&[
        "diff",
        "--compare",
        COUNT_REPORT,
        "tests/fixtures/v4_diff_old.json",
        "tests/fixtures/v4_diff_new.json",
    ]);
    assert_eq!(code, Some(2));
    assert!(out.is_empty(), "{out}");
    assert_eq!(
        err,
        format!(
            "rum-meter: {COUNT_REPORT}: report is a count report; pass a diff report written with --json\n"
        )
    );
}

#[test]
fn a_repeated_path_is_exit_2_without_the_name() {
    let dir = scratch();
    let saved = write(
        &dir,
        "repeat.json",
        br#"{
  "schema_version": 1,
  "tool": "rum-meter",
  "total_rum": 2,
  "by_module": [
    { "name": "module.secret-path", "count": 1 },
    { "name": "module.secret-path", "count": 1 }
  ],
  "by_type": [{ "name": "random_id", "count": 2 }]
}"#,
    );
    let (code, out, err) = run(&["count", "--compare", &saved, ONE]);
    assert_eq!(code, Some(2));
    assert!(out.is_empty(), "{out}");
    assert_eq!(
        err,
        format!(
            "rum-meter: {saved}: report repeats a module path; write the report again with --json\n"
        )
    );
    assert!(!err.contains("module.secret-path"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn format_and_explain_are_refused_and_the_old_checks_stay_first() {
    let (code, out, err) = run(&["count", "--json", "--compare", COUNT_REPORT, ONE]);
    assert_eq!(code, Some(3));
    assert!(out.is_empty(), "{out}");
    assert_eq!(
        err,
        "--compare does not combine with --json, --markdown or --html\n"
    );

    let (code, out, err) = run(&["count", "--explain", "--compare", COUNT_REPORT, ONE]);
    assert_eq!(code, Some(3));
    assert!(out.is_empty(), "{out}");
    assert_eq!(err, "--explain does not combine with --compare\n");

    let (code, out, err) = run(&[
        "count",
        "--max-increase",
        "0",
        "--json",
        "--compare",
        COUNT_REPORT,
        ONE,
    ]);
    assert_eq!(code, Some(3));
    assert!(out.is_empty(), "{out}");
    assert_eq!(err, "--max-increase only works with diff\n");

    let (code, out, err) = run(&[
        "diff",
        "--explain",
        "--compare",
        DIFF_REPORT,
        "tests/fixtures/v4_diff_old.json",
        "tests/fixtures/v4_diff_new.json",
    ]);
    assert_eq!(code, Some(3));
    assert_eq!(err, "--explain with diff only works together with --json\n");
    assert!(out.is_empty(), "{out}");
}
