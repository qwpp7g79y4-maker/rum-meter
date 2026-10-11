//! States produced by OpenTofu from `tests/fixtures/tofu/<scenario>/main.tf`.
//!
//! Expected RUM is taken from those files. It is not copied from rum-meter.
//! `generate.sh` builds the states; these tests do not run tofu.
//!
//! The safety test reads string values under `attributes` only to prove that
//! text, JSON, `--explain` and `diff` omit them. rum-meter never loads them.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const COUNT_STATE: &str = "tests/fixtures/tofu/count/terraform.tfstate";
const FOR_EACH_STATE: &str = "tests/fixtures/tofu/for_each/terraform.tfstate";
const MODULE_STATE: &str = "tests/fixtures/tofu/module/terraform.tfstate";
const DATA_STATE: &str = "tests/fixtures/tofu/data/terraform.tfstate";
const DEPOSED_STATE: &str = "tests/fixtures/tofu/deposed/terraform.tfstate";

/// `count/main.tf`: `random_id.servers` count = 3, plus one `random_password.secret`.
const COUNT_RUM: usize = 4;
const COUNT_R2: usize = 0;
const COUNT_R3: usize = 0;
const COUNT_R4: usize = 0;

/// `for_each/main.tf`: `random_id.labeled` for_each over web, db and api.
const FOR_EACH_RUM: usize = 3;
const FOR_EACH_R2: usize = 0;
const FOR_EACH_R3: usize = 0;
const FOR_EACH_R4: usize = 0;

/// `module/main.tf`: one `random_id.one` in `module.child`, and `random_id.pair` count = 2 in the nested module.
const MODULE_RUM: usize = 3;
const MODULE_R2: usize = 0;
const MODULE_R3: usize = 0;
const MODULE_R4: usize = 0;

/// `data/main.tf`: one managed `random_id.kept`, one `data.local_file.note`,
/// one `null_resource.wait` and one `terraform_data.marker`.
const DATA_RUM: usize = 1;
const DATA_R2: usize = 1;
const DATA_R3: usize = 2;
const DATA_R4: usize = 0;

/// `deposed/main.tf`: one `random_pet.replaced`. The second apply fails after
/// the first object is deposed, leaving that object and the new one.
const DEPOSED_RUM: usize = 2;
const DEPOSED_R2: usize = 0;
const DEPOSED_R3: usize = 0;
const DEPOSED_R4: usize = 0;

struct Counts {
    rum: usize,
    r2: usize,
    r3: usize,
    r4: usize,
}

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
        "exit {}\nstderr bytes: {}\nstdout bytes: {}",
        output.status.code().unwrap_or(-1),
        stderr.len(),
        stdout.len()
    );
    assert!(stderr.is_empty(), "stderr bytes: {}", stderr.len());
    stdout
}

fn assert_count(path: &str, counts: Counts) {
    let stdout = stdout_ok(&run(&["count", path]));
    let file_block = format!(
        "file: {path}\nrum: {}\nnot counted: R2 {}, R3 {}, R4 {}\n",
        counts.rum, counts.r2, counts.r3, counts.r4
    );
    assert!(
        stdout.starts_with(&file_block),
        "per-file tally mismatch ({} bytes)",
        stdout.len()
    );
    let total = format!(
        "total rum: {}\nnot counted: R2 {}, R3 {}, R4 {}\n",
        counts.rum, counts.r2, counts.r3, counts.r4
    );
    assert!(
        stdout.contains(&total),
        "total tally mismatch ({} bytes)",
        stdout.len()
    );

    let explained = stdout_ok(&run(&["count", "--explain", path]));
    let (r1, r2, r3, r4) = explain_counts(&explained);
    assert_eq!(r1, counts.rum, "explain R1");
    assert_eq!(r2, counts.r2, "explain R2");
    assert_eq!(r3, counts.r3, "explain R3");
    assert_eq!(r4, counts.r4, "explain R4");

    let json = stdout_ok(&run(&["count", "--json", path]));
    let value: serde_json::Value = serde_json::from_str(&json).expect("count json");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["total_rum"], counts.rum);
}

fn explain_counts(stdout: &str) -> (usize, usize, usize, usize) {
    let mut r1 = 0;
    let mut r2 = 0;
    let mut r3 = 0;
    let mut r4 = 0;
    for line in stdout.lines() {
        let line = line.trim();
        if line.ends_with("counts R1") || line.ends_with("counts R1 tainted") {
            r1 += 1;
        } else if line.ends_with("does not count R2") {
            r2 += 1;
        } else if line.ends_with("does not count R3") {
            r3 += 1;
        } else if line.ends_with("does not count R4") {
            r4 += 1;
        }
    }
    (r1, r2, r3, r4)
}

// count/main.tf: random_id.servers count = 3, plus one random_password.secret → 4 RUM, R2 0, R3 0, R4 0.
#[test]
fn count_scenario_matches_main_tf() {
    assert_count(
        COUNT_STATE,
        Counts {
            rum: COUNT_RUM,
            r2: COUNT_R2,
            r3: COUNT_R3,
            r4: COUNT_R4,
        },
    );
    let explained = stdout_ok(&run(&["count", "--explain", COUNT_STATE]));
    for address in [
        "random_id.servers[0] counts R1",
        "random_id.servers[1] counts R1",
        "random_id.servers[2] counts R1",
        "random_password.secret counts R1",
    ] {
        assert!(
            explained.contains(address),
            "explain is missing the instance declared in count/main.tf"
        );
    }
}

// for_each/main.tf: random_id.labeled for_each over web, db and api → 3 RUM, R2 0, R3 0, R4 0.
#[test]
fn for_each_scenario_matches_main_tf() {
    assert_count(
        FOR_EACH_STATE,
        Counts {
            rum: FOR_EACH_RUM,
            r2: FOR_EACH_R2,
            r3: FOR_EACH_R3,
            r4: FOR_EACH_R4,
        },
    );
    let explained = stdout_ok(&run(&["count", "--explain", FOR_EACH_STATE]));
    for address in [
        "random_id.labeled[\"api\"] counts R1",
        "random_id.labeled[\"db\"] counts R1",
        "random_id.labeled[\"web\"] counts R1",
    ] {
        assert!(
            explained.contains(address),
            "explain is missing the instance declared in for_each/main.tf"
        );
    }
}

// module/main.tf: random_id.one in module.child, random_id.pair count = 2 in the nested module → 3 RUM, R2 0, R3 0, R4 0.
#[test]
fn module_scenario_matches_main_tf() {
    assert_count(
        MODULE_STATE,
        Counts {
            rum: MODULE_RUM,
            r2: MODULE_R2,
            r3: MODULE_R3,
            r4: MODULE_R4,
        },
    );
    let explained = stdout_ok(&run(&["count", "--explain", MODULE_STATE]));
    for address in [
        "module.child.random_id.one counts R1",
        "module.child.module.nested.random_id.pair[0] counts R1",
        "module.child.module.nested.random_id.pair[1] counts R1",
    ] {
        assert!(
            explained.contains(address),
            "explain is missing the instance declared in module/main.tf"
        );
    }
}

// data/main.tf: random_id.kept counts; local_file.note is data; null_resource.wait and terraform_data.marker do not → 1 RUM, R2 1, R3 2, R4 0.
#[test]
fn data_scenario_matches_main_tf() {
    assert_count(
        DATA_STATE,
        Counts {
            rum: DATA_RUM,
            r2: DATA_R2,
            r3: DATA_R3,
            r4: DATA_R4,
        },
    );
    let explained = stdout_ok(&run(&["count", "--explain", DATA_STATE]));
    for address in [
        "random_id.kept counts R1",
        "data.local_file.note does not count R2",
        "null_resource.wait does not count R3",
        "terraform_data.marker does not count R3",
    ] {
        assert!(
            explained.contains(address),
            "explain is missing the instance declared in data/main.tf"
        );
    }
}

// deposed/main.tf: one random_pet.replaced. The failed second apply keeps the previous object deposed → 2 RUM, R2 0, R3 0, R4 0.
#[test]
fn deposed_scenario_matches_main_tf() {
    assert_count(
        DEPOSED_STATE,
        Counts {
            rum: DEPOSED_RUM,
            r2: DEPOSED_R2,
            r3: DEPOSED_R3,
            r4: DEPOSED_R4,
        },
    );
    let explained = stdout_ok(&run(&["count", "--explain", DEPOSED_STATE]));
    let lines: Vec<&str> = explained.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"random_pet.replaced counts R1 tainted"),
        "explain is missing the current random_pet.replaced"
    );
    let deposed = lines
        .iter()
        .filter(|line| line.contains("(deposed ") && line.ends_with("counts R1"))
        .count();
    assert_eq!(deposed, 1, "explain should show one deposed random_pet");
}

fn manifest(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn tofu_states() -> Vec<String> {
    let root = manifest("tests/fixtures/tofu");
    let mut paths = Vec::new();
    for entry in fs::read_dir(&root).expect("tofu fixture dir") {
        let path = entry.expect("dir entry").path();
        if !path.is_dir() {
            continue;
        }
        let state = path.join("terraform.tfstate");
        if state.is_file() {
            let rel = state
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .expect("state path is inside the crate");
            paths.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    paths.sort();
    assert!(!paths.is_empty(), "no tofu states");
    paths
}

fn push_strings(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => found.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                push_strings(item, found);
            }
        }
        serde_json::Value::Object(map) => {
            for child in map.values() {
                push_strings(child, found);
            }
        }
        _ => {}
    }
}

struct StateSecrets {
    path: String,
    values: Vec<String>,
    sensitive: bool,
}

fn load_state(rel: &str) -> StateSecrets {
    let text = fs::read_to_string(manifest(rel)).unwrap_or_else(|_| panic!("read {rel}"));
    let value: serde_json::Value = serde_json::from_str(&text).expect("tofu state is json");
    assert_eq!(value["version"], 4, "{rel} is not state v4");
    let resources = value["resources"].as_array().expect("resources");
    let mut values = Vec::new();
    let mut sensitive = false;
    for resource in resources {
        let instances = resource["instances"].as_array().expect("instances");
        for instance in instances {
            if let Some(attributes) = instance.get("attributes") {
                push_strings(attributes, &mut values);
            }
            if instance
                .get("sensitive_attributes")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|items| !items.is_empty())
            {
                sensitive = true;
            }
        }
    }
    StateSecrets {
        path: rel.to_string(),
        values,
        sensitive,
    }
}

fn assert_values_absent(label: &str, output: &str, values: &[String]) {
    for value in values {
        if value.is_empty() {
            continue;
        }
        assert!(
            !output.contains(value.as_str()),
            "{label}: output contains a state attribute string of {} bytes",
            value.len()
        );
    }
}

fn diff_pairs(paths: &[String]) -> Vec<(String, String)> {
    if paths.len() == 1 {
        return vec![(paths[0].clone(), paths[0].clone())];
    }
    paths
        .windows(2)
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect()
}

#[test]
fn tofu_attribute_strings_stay_out_of_every_report() {
    let states: Vec<StateSecrets> = tofu_states().iter().map(|path| load_state(path)).collect();
    assert!(
        states.iter().any(|state| state.sensitive),
        "a tofu state must carry a real sensitive attribute"
    );
    assert!(
        states
            .iter()
            .flat_map(|state| state.values.iter())
            .any(|value| value.len() >= 32),
        "expected a long attribute string from random_password"
    );

    for state in &states {
        for args in [
            vec!["count", state.path.as_str()],
            vec!["count", "--explain", state.path.as_str()],
            vec!["count", "--json", state.path.as_str()],
            vec!["count", "--json", "--explain", state.path.as_str()],
        ] {
            let output = run(&args);
            let stdout = stdout_ok(&output);
            let label = args.join(" ");
            assert_values_absent(&label, &stdout, &state.values);
            let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
            assert_values_absent(&label, &stderr, &state.values);
        }
    }

    let paths: Vec<String> = states.iter().map(|state| state.path.clone()).collect();
    for (old, new) in diff_pairs(&paths) {
        let values: Vec<String> = states
            .iter()
            .filter(|state| state.path == old || state.path == new)
            .flat_map(|state| state.values.iter().cloned())
            .collect();
        for args in [
            vec!["diff", old.as_str(), new.as_str()],
            vec!["diff", "--json", old.as_str(), new.as_str()],
            vec!["diff", "--json", "--explain", old.as_str(), new.as_str()],
        ] {
            let output = run(&args);
            let stdout = stdout_ok(&output);
            let label = args.join(" ");
            assert_values_absent(&label, &stdout, &values);
            let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
            assert_values_absent(&label, &stderr, &values);
        }
        let refused = vec!["diff", "--explain", old.as_str(), new.as_str()];
        let output = run(&refused);
        let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is utf-8");
        let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
        assert!(
            output.status.code() == Some(3)
                && stdout.is_empty()
                && stderr == "--explain with diff only works together with --json\n",
            "diff --explain: exit {:?} stdout bytes {} stderr bytes {}",
            output.status.code(),
            stdout.len(),
            stderr.len()
        );
        let label = refused.join(" ");
        assert_values_absent(&label, &stdout, &values);
        assert_values_absent(&label, &stderr, &values);
    }
}
