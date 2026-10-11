//! Object status in one handwritten state: a current instance, a tainted
//! instance and a deposed instance.
//!
//! The three counting addresses are distinct. `count` shows them in text and
//! JSON. `diff` against a state that only has the current object adds the
//! deposed object and the tainted object, and that difference equals the RUM
//! delta.

use std::path::PathBuf;
use std::process::{Command, Output};

const FIXTURE: &str = "tests/fixtures/v4_object_status.json";
const ESTIMATE: &str = "estimate: HCP Terraform bills the hourly peak; this is a single snapshot";

const COUNT_EXPLAIN: &str = "\
file: tests/fixtures/v4_object_status.json
rum: 3
not counted: R2 0, R3 0, R4 0
  local_file.a counts R1
  local_file.a (deposed 00000001) counts R1
  local_file.b counts R1 tainted
total rum: 3
not counted: R2 0, R3 0, R4 0
modules:
  (root): 3
types:
  local_file: 3
estimate: HCP Terraform bills the hourly peak; this is a single snapshot
";

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
    assert!(!stdout.contains("CANARY-SECRET"), "leaked:\n{stdout}");
    assert!(!stderr.contains("CANARY-SECRET"), "leaked:\n{stderr}");
    stdout
}

fn listed(stdout: &str, header: &str) -> Vec<String> {
    let mut lines = stdout.lines();
    let header = format!("{header}:");
    for line in lines.by_ref() {
        if line == header {
            break;
        }
    }
    let mut items = Vec::new();
    for line in lines {
        if let Some(item) = line.strip_prefix("  ") {
            items.push(item.to_string());
        } else {
            break;
        }
    }
    items
}

fn write_current_only() -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("object-status");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("current-only.json");
    std::fs::write(
        &path,
        r#"{
  "version": 4,
  "terraform_version": "1.9.8",
  "serial": 1,
  "lineage": "synthetic-handwritten-os-old",
  "outputs": {},
  "resources": [
    {
      "mode": "managed",
      "type": "local_file",
      "name": "a",
      "provider": "provider[\"registry.terraform.io/hashicorp/local\"]",
      "instances": [{ "schema_version": 0, "attributes": { "id": "current" } }]
    }
  ]
}
"#,
    )
    .expect("write old state");
    path.into_os_string().into_string().expect("utf-8")
}

#[test]
fn count_text_shows_three_rum_and_three_addresses() {
    let plain = stdout_ok(&run(&["count", FIXTURE]));
    assert!(plain.contains("rum: 3\n"), "plain:\n{plain}");
    assert!(plain.contains("total rum: 3\n"), "plain:\n{plain}");
    assert_eq!(plain.matches(ESTIMATE).count(), 1);
    assert!(
        !plain.contains("deposed"),
        "plain text lists no addresses:\n{plain}"
    );
    assert!(
        !plain.contains("tainted"),
        "plain text lists no addresses:\n{plain}"
    );

    let explained = stdout_ok(&run(&["count", "--explain", FIXTURE]));
    assert_eq!(explained, COUNT_EXPLAIN);
}

#[test]
fn count_json_marks_tainted_and_deposed_without_raising_the_schema() {
    let stdout = stdout_ok(&run(&["count", "--json", "--explain", FIXTURE]));
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["total_rum"], 3);
    let instances = value["instances"].as_array().expect("instances");
    assert_eq!(instances.len(), 3);

    let plain = &instances[0];
    assert_eq!(plain["address"], "local_file.a");
    assert_eq!(plain["counts"], true);
    assert_eq!(plain["rule"], "R1");
    assert!(plain.get("deposed").is_none());
    assert!(plain.get("tainted").is_none());

    let deposed = &instances[1];
    assert_eq!(deposed["address"], "local_file.a (deposed 00000001)");
    assert_eq!(deposed["counts"], true);
    assert_eq!(deposed["rule"], "R1");
    assert_eq!(deposed["deposed"], "00000001");
    assert!(deposed.get("tainted").is_none());

    let tainted = &instances[2];
    assert_eq!(tainted["address"], "local_file.b");
    assert_eq!(tainted["counts"], true);
    assert_eq!(tainted["rule"], "R1");
    assert_eq!(tainted["tainted"], true);
    assert!(tainted.get("deposed").is_none());

    let addresses: Vec<&str> = instances
        .iter()
        .map(|instance| instance["address"].as_str().expect("address"))
        .collect();
    let mut unique = addresses.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 3, "addresses: {addresses:?}");
}

#[test]
fn diff_added_minus_removed_equals_rum_delta_with_deposed_objects() {
    let old = write_current_only();
    let stdout = stdout_ok(&run(&["diff", &old, FIXTURE]));
    assert!(stdout.contains("rum: 1 -> 3\n"), "diff:\n{stdout}");
    assert!(stdout.contains("rum delta: +2\n"), "diff:\n{stdout}");
    let added = listed(&stdout, "added");
    let removed = listed(&stdout, "removed");
    assert_eq!(
        added.len() as i64 - removed.len() as i64,
        2,
        "added {added:?} removed {removed:?}\n{stdout}"
    );
    assert_eq!(
        added,
        vec![
            "local_file.a (deposed 00000001)".to_string(),
            "local_file.b".to_string(),
        ]
    );
    assert!(removed.is_empty(), "removed {removed:?}");
    assert!(
        listed(&stdout, "changed rule").is_empty(),
        "a deposed object is not a rule change"
    );

    let json = stdout_ok(&run(&["diff", "--json", &old, FIXTURE]));
    let value: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["rum_delta"], 2);
    assert_eq!(value["total_rum"]["old"], 1);
    assert_eq!(value["total_rum"]["new"], 3);
    let added = value["added"].as_array().expect("added");
    let removed = value["removed"].as_array().expect("removed");
    let delta = value["rum_delta"].as_i64().expect("rum_delta");
    assert_eq!(added.len() as i64 - removed.len() as i64, delta);
    assert_eq!(
        added
            .iter()
            .map(|item| item.as_str().expect("address"))
            .collect::<Vec<_>>(),
        vec!["local_file.a (deposed 00000001)", "local_file.b"]
    );
    assert!(removed.is_empty());
}
