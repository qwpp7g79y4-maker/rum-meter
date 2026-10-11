//! Binary tests for `diff` on two handwritten states.
//!
//! Old counting set: `random_id.servers[0]`, `random_id.steady`,
//! `random_id.marker`, `local_file.note`, and
//! `module.child.random_id.labeled` keys `web` and `gone` (6).
//! New counting set: `servers[0]`, `servers[1]`, `steady`, and `labeled["web"]` (4).
//! `gone` is removed, `marker` becomes `terraform_data` (the old address is
//! removed), and managed `local_file.note` becomes `data.local_file.note`,
//! so the counting address is removed. Delta is -2.

use std::path::PathBuf;
use std::process::{Command, Output};

const ESTIMATE: &str = "estimate: HCP Terraform bills the hourly peak; this is a single snapshot";

const BODY: &str = r#"old: tests/fixtures/v4_diff_old.json
new: tests/fixtures/v4_diff_new.json
rum: 6 -> 4
rum delta: -2
added:
  random_id.servers[1]
removed:
  local_file.note
  module.child.random_id.labeled["gone"]
  random_id.marker
changed rule:
"#;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary")
}

fn report(costs: &str) -> String {
    let mut text = BODY.to_string();
    text.push_str(costs);
    text.push_str(ESTIMATE);
    text.push('\n');
    text
}

fn write_prices() -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("diff");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("tiers.json");
    std::fs::write(
        &path,
        r#"{"currency":"EUR","note":"CANARY-SECRET-D-PRICE","tiers":[{"name":"alpha","price_per_rum_hour":1.25},{"name":"beta","price_per_rum_hour":0.5}]}"#,
    )
    .expect("write price file");
    path.into_os_string().into_string().expect("utf-8")
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
    assert!(!stdout.contains("CANARY-SECRET"), "leaked:\n{stdout}");
    assert!(!stderr.contains("CANARY-SECRET"), "leaked:\n{stderr}");
    stdout
}

#[test]
fn two_fixture_states_show_added_removed_rule_change_and_rum_delta() {
    let stdout = stdout_ok(&run(&[
        "diff",
        "tests/fixtures/v4_diff_old.json",
        "tests/fixtures/v4_diff_new.json",
    ]));
    assert_eq!(stdout, report(""));
    assert!(!stdout.contains("cost delta:"));
    assert!(!stdout.contains("EUR"));
}

#[test]
fn prices_show_the_monthly_cost_delta_per_tier() {
    let prices = write_prices();
    let old = "tests/fixtures/v4_diff_old.json";
    let new = "tests/fixtures/v4_diff_new.json";

    let default_hours = stdout_ok(&run(&["diff", old, new, "--prices", &prices]));
    assert_eq!(
        default_hours,
        report("cost delta:\n  alpha: EUR -1825.00\n  beta: EUR -730.00\n")
    );

    let ten_hours = stdout_ok(&run(&[
        "diff",
        "--hours-per-month",
        "10",
        old,
        new,
        "--prices",
        &prices,
    ]));
    assert_eq!(
        ten_hours,
        report("cost delta:\n  alpha: EUR -25.00\n  beta: EUR -10.00\n")
    );
}
