//! `--top` remainder must not reuse a real module or type name `other`.
//!
//! Reported: five managed resources in module `other`, one in `zzz`, `--top 1`
//! produced two `other:` rows. Variants cover type name `other`, JSON output,
//! CRLF / tab / LF around the module field, and a `\uXXXX`-escaped type name
//! that decodes to `other`.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rum-top-other-{nanos}-{n}"));
    fs::create_dir_all(&dir).expect("temp dir");
    dir.join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary")
}

fn assert_ok(output: &Output) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).expect("utf8");
    let stderr = String::from_utf8(output.stderr.clone()).expect("utf8");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    stdout
}

fn names_under(stdout: &str, header: &str) -> Vec<String> {
    let mut lines = stdout.lines();
    let wanted = format!("{header}:");
    assert!(
        lines.any(|line| line == wanted),
        "missing {header} in:\n{stdout}"
    );
    lines
        .take_while(|line| line.starts_with("  "))
        .map(|line| {
            line.trim_start()
                .rsplit_once(": ")
                .expect("name: count")
                .0
                .to_string()
        })
        .collect()
}

fn assert_unique(names: &[String]) {
    let mut sorted = names.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(names.len(), sorted.len(), "duplicate names in {names:?}");
}

fn write_state(path: &PathBuf, body: &str) {
    fs::write(path, body).expect("write state");
}

#[test]
fn module_named_other_uses_paren_other_for_remainder() {
    // Exact reported shape: module "other" ×5, module "zzz" ×1, --top 1.
    let path = scratch("mod-other.json");
    let mut resources = String::new();
    for i in 0..5 {
        resources.push_str(&format!(
            r#"{{"module":"other","mode":"managed","type":"random_id","name":"a{i}","provider":"provider[\"registry.terraform.io/hashicorp/random\"]","instances":[{{"schema_version":0,"attributes":{{}}}}]}},"#
        ));
    }
    resources.push_str(
        r#"{"module":"zzz","mode":"managed","type":"random_id","name":"z","provider":"provider[\"registry.terraform.io/hashicorp/random\"]","instances":[{"schema_version":0,"attributes":{}}]}"#,
    );
    write_state(
        &path,
        &format!(
            r#"{{"version":4,"terraform_version":"1.9.8","serial":1,"lineage":"t","outputs":{{}},"resources":[{resources}]}}"#
        ),
    );
    let stdout = assert_ok(&run(&["count", "--top", "1", path.to_str().unwrap()]));
    let modules = names_under(&stdout, "modules");
    assert_eq!(modules, vec!["other".to_string(), "(other)".to_string()]);
    assert_unique(&modules);
    assert!(stdout.contains("other: 5"));
    assert!(stdout.contains("(other): 1"));
}

#[test]
fn type_named_other_uses_paren_other_for_remainder() {
    let path = scratch("type-other.json");
    let mut resources = String::new();
    for i in 0..5 {
        resources.push_str(&format!(
            r#"{{"mode":"managed","type":"other","name":"a{i}","provider":"provider[\"registry.terraform.io/hashicorp/null\"]","instances":[{{"schema_version":0,"attributes":{{}}}}]}},"#
        ));
    }
    resources.push_str(
        r#"{"mode":"managed","type":"zzz","name":"z","provider":"provider[\"registry.terraform.io/hashicorp/null\"]","instances":[{"schema_version":0,"attributes":{}}]}"#,
    );
    write_state(
        &path,
        &format!(
            r#"{{"version":4,"terraform_version":"1.9.8","serial":1,"lineage":"t","outputs":{{}},"resources":[{resources}]}}"#
        ),
    );
    let stdout = assert_ok(&run(&["count", "--top", "1", path.to_str().unwrap()]));
    let types = names_under(&stdout, "types");
    assert_eq!(types, vec!["other".to_string(), "(other)".to_string()]);
    assert_unique(&types);
}

#[test]
fn json_output_keeps_unique_names_for_other_collision() {
    let path = scratch("json-other.json");
    let mut resources = String::new();
    for i in 0..5 {
        resources.push_str(&format!(
            r#"{{"module":"other","mode":"managed","type":"random_id","name":"a{i}","provider":"provider[\"registry.terraform.io/hashicorp/random\"]","instances":[{{"schema_version":0,"attributes":{{}}}}]}},"#
        ));
    }
    resources.push_str(
        r#"{"module":"zzz","mode":"managed","type":"random_id","name":"z","provider":"provider[\"registry.terraform.io/hashicorp/random\"]","instances":[{"schema_version":0,"attributes":{}}]}"#,
    );
    write_state(
        &path,
        &format!(
            r#"{{"version":4,"terraform_version":"1.9.8","serial":1,"lineage":"t","outputs":{{}},"resources":[{resources}]}}"#
        ),
    );
    let stdout = assert_ok(&run(&[
        "count",
        "--top",
        "1",
        "--json",
        path.to_str().unwrap(),
    ]));
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    let modules = value["by_module"].as_array().expect("by_module");
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0]["name"], "other");
    assert_eq!(modules[0]["count"], 5);
    assert_eq!(modules[1]["name"], "(other)");
    assert_eq!(modules[1]["count"], 1);
}

#[test]
fn whitespace_and_unicode_escape_variants_still_unique() {
    // LF / CR / tab around fields; type name via \u006fther (= "other").
    let cases = [
        (
            "lf.json",
            "{\n\"version\":4,\n\"terraform_version\":\"1.9.8\",\n\"serial\":1,\n\"lineage\":\"t\",\n\"outputs\":{},\n\"resources\":[\n{\"module\":\"other\",\"mode\":\"managed\",\"type\":\"random_id\",\"name\":\"a0\",\"provider\":\"provider[\\\"registry.terraform.io/hashicorp/random\\\"]\",\"instances\":[{\"schema_version\":0,\"attributes\":{}}]},\n{\"module\":\"zzz\",\"mode\":\"managed\",\"type\":\"random_id\",\"name\":\"z\",\"provider\":\"provider[\\\"registry.terraform.io/hashicorp/random\\\"]\",\"instances\":[{\"schema_version\":0,\"attributes\":{}}]}\n]}\n",
        ),
        (
            "crlf.json",
            "{\r\n\"version\":4,\r\n\"terraform_version\":\"1.9.8\",\r\n\"serial\":1,\r\n\"lineage\":\"t\",\r\n\"outputs\":{},\r\n\"resources\":[\r\n{\"module\":\"other\",\"mode\":\"managed\",\"type\":\"random_id\",\"name\":\"a0\",\"provider\":\"provider[\\\"registry.terraform.io/hashicorp/random\\\"]\",\"instances\":[{\"schema_version\":0,\"attributes\":{}}]},\r\n{\"module\":\"zzz\",\"mode\":\"managed\",\"type\":\"random_id\",\"name\":\"z\",\"provider\":\"provider[\\\"registry.terraform.io/hashicorp/random\\\"]\",\"instances\":[{\"schema_version\":0,\"attributes\":{}}]}\r\n]}\r\n",
        ),
        (
            "tab.json",
            "{\t\"version\":4,\t\"terraform_version\":\"1.9.8\",\t\"serial\":1,\t\"lineage\":\"t\",\t\"outputs\":{},\t\"resources\":[\t{\"module\":\"other\",\"mode\":\"managed\",\"type\":\"random_id\",\"name\":\"a0\",\"provider\":\"provider[\\\"registry.terraform.io/hashicorp/random\\\"]\",\"instances\":[{\"schema_version\":0,\"attributes\":{}}]},\t{\"module\":\"zzz\",\"mode\":\"managed\",\"type\":\"random_id\",\"name\":\"z\",\"provider\":\"provider[\\\"registry.terraform.io/hashicorp/random\\\"]\",\"instances\":[{\"schema_version\":0,\"attributes\":{}}]}\t]}\t",
        ),
        (
            "u-escape.json",
            r#"{"version":4,"terraform_version":"1.9.8","serial":1,"lineage":"t","outputs":{},"resources":[{"mode":"managed","type":"\u006fther","name":"a0","provider":"provider[\"registry.terraform.io/hashicorp/null\"]","instances":[{"schema_version":0,"attributes":{}}]},{"mode":"managed","type":"zzz","name":"z","provider":"provider[\"registry.terraform.io/hashicorp/null\"]","instances":[{"schema_version":0,"attributes":{}}]}]}"#,
        ),
    ];
    for (name, body) in cases {
        let path = scratch(name);
        write_state(&path, body);
        let stdout = assert_ok(&run(&["count", "--top", "1", path.to_str().unwrap()]));
        if name == "u-escape.json" {
            let types = names_under(&stdout, "types");
            assert_eq!(
                types,
                vec!["other".to_string(), "(other)".to_string()],
                "{name}"
            );
            assert_unique(&types);
        } else {
            let modules = names_under(&stdout, "modules");
            assert_eq!(
                modules,
                vec!["other".to_string(), "(other)".to_string()],
                "{name}"
            );
            assert_unique(&modules);
        }
    }
}
