//! `diff` matches objects on their exact identity, not on the printed address.
//! Regression: a resource named `a[0]` and resource `a` at index 0 both print
//! as `random_id.a[0]`, and `diff` used to report no change.

use std::process::Command;

fn diff(old: &str, new: &str) -> serde_json::Value {
    let dir = std::env::temp_dir().join(format!(
        "rum-meter-identity-{}-{}",
        std::process::id(),
        old.len() * 31 + new.len()
    ));
    std::fs::create_dir_all(&dir).expect("dir");
    let old_path = dir.join("old.json");
    let new_path = dir.join("new.json");
    std::fs::write(&old_path, old).expect("old");
    std::fs::write(&new_path, new).expect("new");
    let out = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(["diff", "--json"])
        .arg(&old_path)
        .arg(&new_path)
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("json")
}

fn state(resources: &str) -> String {
    format!(r#"{{"version":4,"resources":[{resources}]}}"#)
}

fn list(value: &serde_json::Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .expect(key)
        .iter()
        .map(|item| {
            item.as_str()
                .map_or_else(|| item.to_string(), str::to_string)
        })
        .collect()
}

/// Pairs of different objects that print the same address. Each must be one
/// removal and one addition, in both directions.
#[test]
fn lookalike_addresses_are_removed_and_added() {
    let pairs = [
        (
            "name with index",
            r#"{"mode":"managed","type":"random_id","name":"a[0]","instances":[{}]}"#,
            r#"{"mode":"managed","type":"random_id","name":"a","instances":[{"index_key":0}]}"#,
        ),
        (
            "name with key",
            r#"{"mode":"managed","type":"random_id","name":"a[\"k\"]","instances":[{}]}"#,
            r#"{"mode":"managed","type":"random_id","name":"a","instances":[{"index_key":"k"}]}"#,
        ),
        (
            "number vs string key",
            r#"{"mode":"managed","type":"random_id","name":"a","instances":[{"index_key":"0"}]}"#,
            r#"{"mode":"managed","type":"random_id","name":"a","instances":[{"index_key":0}]}"#,
        ),
        (
            "dot in type",
            r#"{"mode":"managed","type":"random_id.b","name":"c","instances":[{}]}"#,
            r#"{"mode":"managed","type":"random_id","name":"b.c","instances":[{}]}"#,
        ),
        (
            "module vs type",
            r#"{"module":"module.x","mode":"managed","type":"random_id","name":"a","instances":[{}]}"#,
            r#"{"mode":"managed","type":"module.x.random_id","name":"a","instances":[{}]}"#,
        ),
        (
            "deposed in name",
            r#"{"mode":"managed","type":"random_id","name":"a (deposed d1)","instances":[{}]}"#,
            r#"{"mode":"managed","type":"random_id","name":"a","instances":[{"deposed":"d1"}]}"#,
        ),
    ];
    for (name, left, right) in pairs {
        for (old, new) in [(left, right), (right, left)] {
            let value = diff(&state(old), &state(new));
            let added = list(&value, "added");
            let removed = list(&value, "removed");
            assert_eq!(added.len(), 1, "{name}: {value}");
            assert_eq!(removed.len(), 1, "{name}: {value}");
            assert_eq!(value["rum_delta"], 0, "{name}");
        }
    }
}

/// A data source and a managed resource of the same type and name differ.
#[test]
fn data_and_managed_differ_in_module_named_data() {
    let old = state(
        r#"{"module":"module.x.data","mode":"managed","type":"y","name":"n","instances":[{}]}"#,
    );
    let new =
        state(r#"{"module":"module.x","mode":"data","type":"y","name":"n","instances":[{}]}"#);
    let value = diff(&old, &new);
    assert_eq!(list(&value, "removed").len(), 1, "{value}");
    assert_eq!(value["rum_delta"], -1, "{value}");
}

/// The same object in both files is still no change, and a real rule change
/// on the same object is still a rule change.
#[test]
fn same_object_is_still_matched() {
    let a = r#"{"module":"module.m","mode":"managed","type":"random_id","name":"a","instances":[{"index_key":"k"},{"index_key":"k","deposed":"d"}]}"#;
    let value = diff(&state(a), &state(a));
    assert!(list(&value, "added").is_empty(), "{value}");
    assert!(list(&value, "removed").is_empty(), "{value}");
    assert!(list(&value, "changed_rule").is_empty(), "{value}");
    let managed = state(r#"{"mode":"managed","type":"random_id","name":"a","instances":[{}]}"#);
    let null = state(r#"{"mode":"managed","type":"null_resource","name":"a","instances":[{}]}"#);
    let value = diff(&managed, &null);
    assert_eq!(
        list(&value, "removed").len(),
        1,
        "type change is another object: {value}"
    );
    let empty = state(r#"{"mode":"managed","type":"random_id","name":"a","instances":[]}"#);
    let value = diff(&managed, &empty);
    assert_eq!(
        list(&value, "changed_rule").len(),
        1,
        "R1 to R4 on one object: {value}"
    );
}
