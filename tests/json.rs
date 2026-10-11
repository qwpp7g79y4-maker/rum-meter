//! JSON reports for `count` and `diff`. Price files are written in the test temp dir.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const ESTIMATE: &str = "HCP Terraform bills the hourly peak; this is a single snapshot";

const COUNT_KEYS: &[&str] = &[
    "schema_version",
    "tool",
    "estimate",
    "inputs",
    "total_rum",
    "by_module",
    "by_type",
];

const DIFF_KEYS: &[&str] = &[
    "schema_version",
    "tool",
    "estimate",
    "inputs",
    "total_rum",
    "by_module",
    "by_type",
    "added",
    "removed",
    "changed_rule",
    "rum_delta",
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary")
}

fn stdout_ok(output: &Output) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is utf-8");
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("CANARY-SECRET"),
        "stdout leaked a fixture value"
    );
    assert!(
        !stderr.contains("CANARY-SECRET"),
        "stderr leaked a fixture value"
    );
    stdout
}

/// `python3`, or `python` where only that name exists (often on Windows).
fn python() -> Command {
    let works = Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success());
    let mut cmd = Command::new(if works { "python3" } else { "python" });
    // Python on Windows reads stdin as the ANSI code page unless told otherwise.
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd
}

fn assert_json_tool(stdout: &str) {
    let mut child = python()
        .args(["-m", "json.tool"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("python3");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(stdout.as_bytes()).expect("write json");
    }
    let result = child.wait_with_output().expect("python3");
    let tool_stderr = String::from_utf8(result.stderr).expect("python stderr is utf-8");
    assert!(
        result.status.success(),
        "python3 -m json.tool rejected the report:\n{tool_stderr}"
    );
    let pretty = String::from_utf8(result.stdout).expect("python stdout is utf-8");
    let original: serde_json::Value = serde_json::from_str(stdout).expect("stdout json");
    let rewritten: serde_json::Value = serde_json::from_str(&pretty).expect("json.tool stdout");
    assert_eq!(original, rewritten);
}

fn assert_keys(value: &serde_json::Value, expected: &[&str]) {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut wanted = expected.to_vec();
    wanted.sort_unstable();
    assert_eq!(keys, wanted);
}

fn snapshot(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("snapshot {}: {err}", path.display()))
}

fn write_prices() -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("json");
    std::fs::create_dir_all(&dir).expect("temp dir");
    // One file per call: tests run in parallel and must not read a half-written file.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("tiers-{n}.json"));
    std::fs::write(
        &path,
        r#"{"currency":"EUR","note":"CANARY-SECRET-J-PRICE","tiers":[{"name":"alpha","price_per_rum_hour":1.25},{"name":"beta","price_per_rum_hour":0.5}]}"#,
    )
    .expect("write price file");
    path.into_os_string().into_string().expect("utf-8")
}

fn assert_report(name: &str, args: &[&str], key_set: &[&str]) -> serde_json::Value {
    let stdout = stdout_ok(&run(args));
    assert_json_tool(&stdout);
    assert_eq!(stdout, snapshot(name), "snapshot {name}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(value["schema_version"].as_u64(), Some(1));
    assert_eq!(value["tool"].as_str(), Some("rum-meter"));
    assert_eq!(value["estimate"].as_str(), Some(ESTIMATE));
    assert_keys(&value, key_set);
    value
}

fn with_extra<'a>(base: &[&'a str], extra: &[&'a str]) -> Vec<&'a str> {
    base.iter().chain(extra.iter()).copied().collect()
}

#[test]
fn count_matches_snapshot() {
    let value = assert_report(
        "count.json",
        &["count", "--json", "tests/fixtures/v4_handwritten.json"],
        COUNT_KEYS,
    );
    assert!(value.get("instances").is_none());
    assert!(value.get("costs").is_none());
    assert_eq!(value["total_rum"].as_u64(), Some(2));
}

#[test]
fn count_explain_matches_snapshot() {
    let extra = ["instances"];
    let keys = with_extra(COUNT_KEYS, &extra);
    let value = assert_report(
        "count_explain.json",
        &[
            "count",
            "--json",
            "--explain",
            "tests/fixtures/v4_handwritten.json",
        ],
        &keys,
    );
    assert!(value.get("costs").is_none());
    assert_eq!(value["instances"].as_array().expect("instances").len(), 6);
}

#[test]
fn count_prices_match_snapshot() {
    let prices = write_prices();
    let extra = ["costs"];
    let keys = with_extra(COUNT_KEYS, &extra);
    let value = assert_report(
        "count_prices.json",
        &[
            "count",
            "--json",
            "--prices",
            &prices,
            "tests/fixtures/v4_handwritten.json",
        ],
        &keys,
    );
    assert!(value.get("instances").is_none());
    assert!(!value.to_string().contains(&prices));
    assert_eq!(value["costs"][0]["amount"].as_str(), Some("1825.00"));
    assert_eq!(value["costs"][1]["amount"].as_str(), Some("730.00"));
}

#[test]
fn diff_matches_snapshot() {
    let value = assert_report(
        "diff.json",
        &[
            "diff",
            "--json",
            "tests/fixtures/v4_diff_old.json",
            "tests/fixtures/v4_diff_new.json",
        ],
        DIFF_KEYS,
    );
    assert!(value.get("instances").is_none());
    assert!(value.get("costs").is_none());
    assert!(value.get("cost_delta").is_none());
    assert_eq!(value["rum_delta"].as_i64(), Some(-2));
    assert_eq!(value["total_rum"]["old"].as_u64(), Some(6));
    assert_eq!(value["total_rum"]["new"].as_u64(), Some(4));
}

#[test]
fn diff_explain_matches_snapshot() {
    let extra = ["instances"];
    let keys = with_extra(DIFF_KEYS, &extra);
    let value = assert_report(
        "diff_explain.json",
        &[
            "diff",
            "--json",
            "--explain",
            "tests/fixtures/v4_diff_old.json",
            "tests/fixtures/v4_diff_new.json",
        ],
        &keys,
    );
    assert!(value.get("costs").is_none());
    assert!(value.get("cost_delta").is_none());
    assert_eq!(value["instances"].as_array().expect("instances").len(), 14);
}

#[test]
fn diff_prices_match_snapshot() {
    let prices = write_prices();
    let extra = ["costs", "cost_delta"];
    let keys = with_extra(DIFF_KEYS, &extra);
    let value = assert_report(
        "diff_prices.json",
        &[
            "diff",
            "--json",
            "--prices",
            &prices,
            "tests/fixtures/v4_diff_old.json",
            "tests/fixtures/v4_diff_new.json",
        ],
        &keys,
    );
    assert!(value.get("instances").is_none());
    assert!(!value.to_string().contains(&prices));
    assert_eq!(value["costs"][0]["amount"].as_str(), Some("3650.00"));
    assert_eq!(value["cost_delta"][0]["amount"].as_str(), Some("-1825.00"));
    assert_eq!(value["cost_delta"][1]["amount"].as_str(), Some("-730.00"));
}

#[test]
fn count_two_files_sums_total_rum() {
    let stdout = stdout_ok(&run(&[
        "count",
        "--json",
        "tests/fixtures/v4_handwritten.json",
        "tests/fixtures/v4_one_managed.json",
    ]));
    assert_json_tool(&stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_keys(&value, COUNT_KEYS);
    assert_eq!(value["total_rum"].as_u64(), Some(3));
    assert_eq!(value["inputs"].as_array().expect("inputs").len(), 2);
    assert_eq!(value["by_module"][0]["name"].as_str(), Some("module.child"));
    assert_eq!(value["by_module"][0]["count"].as_u64(), Some(2));
    assert_eq!(value["by_module"][1]["name"].as_str(), Some("(root)"));
    assert_eq!(value["by_module"][1]["count"].as_u64(), Some(1));
    assert_eq!(value["by_type"][0]["count"].as_u64(), Some(3));
}

#[test]
fn top_merges_the_rest_into_other() {
    let stdout = stdout_ok(&run(&[
        "count",
        "--json",
        "--top",
        "1",
        "tests/fixtures/v4_top.json",
    ]));
    assert_json_tool(&stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_keys(&value, COUNT_KEYS);
    assert_eq!(value["total_rum"].as_u64(), Some(16));
    assert_eq!(
        value["by_module"][0]["name"].as_str(),
        Some("module.compute")
    );
    assert_eq!(value["by_module"][0]["count"].as_u64(), Some(5));
    assert_eq!(value["by_module"][1]["name"].as_str(), Some("other"));
    assert_eq!(value["by_module"][1]["count"].as_u64(), Some(11));
    assert_eq!(value["by_module"].as_array().expect("modules").len(), 2);
    assert_eq!(value["by_type"][0]["name"].as_str(), Some("local_file"));
    assert_eq!(value["by_type"][0]["count"].as_u64(), Some(4));
    assert_eq!(value["by_type"][1]["name"].as_str(), Some("other"));
    assert_eq!(value["by_type"][1]["count"].as_u64(), Some(12));
}

#[test]
fn hours_per_month_scales_json_costs() {
    let prices = write_prices();
    let count = stdout_ok(&run(&[
        "count",
        "--json",
        "--hours-per-month",
        "10",
        "--prices",
        &prices,
        "tests/fixtures/v4_handwritten.json",
    ]));
    assert_json_tool(&count);
    let count: serde_json::Value = serde_json::from_str(&count).expect("json");
    assert_eq!(count["costs"][0]["amount"].as_str(), Some("25.00"));
    assert_eq!(count["costs"][1]["amount"].as_str(), Some("10.00"));

    let diff = stdout_ok(&run(&[
        "diff",
        "--json",
        "--hours-per-month",
        "10",
        "--prices",
        &prices,
        "tests/fixtures/v4_diff_old.json",
        "tests/fixtures/v4_diff_new.json",
    ]));
    assert_json_tool(&diff);
    let diff: serde_json::Value = serde_json::from_str(&diff).expect("json");
    assert_eq!(diff["costs"][0]["amount"].as_str(), Some("50.00"));
    assert_eq!(diff["cost_delta"][0]["amount"].as_str(), Some("-25.00"));
    assert_eq!(diff["cost_delta"][1]["amount"].as_str(), Some("-10.00"));
}

#[test]
fn explain_and_prices_are_both_present_when_both_flags_are_set() {
    let prices = write_prices();
    let stdout = stdout_ok(&run(&[
        "count",
        "--json",
        "--explain",
        "--prices",
        &prices,
        "tests/fixtures/v4_handwritten.json",
    ]));
    assert_json_tool(&stdout);
    assert!(!stdout.contains(&prices));
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    let extra = ["instances", "costs"];
    let keys = with_extra(COUNT_KEYS, &extra);
    assert_keys(&value, &keys);
    assert_eq!(value["instances"].as_array().expect("instances").len(), 6);
    assert_eq!(value["costs"][0]["currency"].as_str(), Some("EUR"));
}
