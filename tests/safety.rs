//! Planted fixture values never reach stdout or stderr.
//!
//! The two state files are synthetic state v4. The marker is in `attributes`,
//! `sensitive_attributes`, `private` and `outputs`. The broken file is not
//! valid JSON and carries the same marker. Identity fields stay free of it,
//! because mode, type, name, module and index_key are part of an address.
//!
//! The other test reads Cargo.lock and `cargo tree`, and rejects network
//! crates. It also rejects `std::net` under `src/`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const OLD: &str = "tests/fixtures/v4_canary_old.json";
const NEW: &str = "tests/fixtures/v4_canary_new.json";
const BROKEN: &str = "tests/fixtures/broken_canary.json";

const REQUIRED: &[&str] = &[
    "CANARY-SECRET-SF-ATTR",
    "CANARY-SECRET-SF-SENSITIVE",
    "CANARY-SECRET-SF-PRIVATE",
    "CANARY-SECRET-SF-OUTPUT",
    "CANARY-SECRET-SF-BROKEN",
];

const NETWORK_ROOTS: &[&str] = &["reqwest", "hyper", "ureq", "tokio"];

#[derive(Clone, Copy)]
struct Expect {
    code: i32,
    stdout_has: &'static [&'static str],
    stderr_has: &'static [&'static str],
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary")
}

fn manifest_path(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn read_fixture(rel: &str) -> String {
    fs::read_to_string(manifest_path(rel)).unwrap_or_else(|_| panic!("read {rel}"))
}

fn planted_tokens(text: &str) -> BTreeSet<String> {
    let marker = "CANARY-SECRET";
    let mut tokens = BTreeSet::new();
    let mut rest = text;
    while let Some(index) = rest.find(marker) {
        let tail = &rest[index..];
        let end = tail
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-'))
            .unwrap_or(tail.len());
        tokens.insert(tail[..end].to_string());
        rest = &tail[end..];
    }
    tokens
}

fn subtree_has_marker(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains("CANARY-SECRET"),
        serde_json::Value::Array(items) => items.iter().any(subtree_has_marker),
        serde_json::Value::Object(map) => map.values().any(subtree_has_marker),
        _ => false,
    }
}

fn field_has_marker(value: &serde_json::Value, field: &str) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            if map.get(field).is_some_and(subtree_has_marker) {
                return true;
            }
            map.values().any(|child| field_has_marker(child, field))
        }
        serde_json::Value::Array(items) => items.iter().any(|item| field_has_marker(item, field)),
        _ => false,
    }
}

fn assert_identity_is_clean(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let identity = matches!(
                    key.as_str(),
                    "mode" | "type" | "name" | "module" | "index_key"
                );
                if let (true, serde_json::Value::String(text)) = (identity, child) {
                    assert!(
                        !text.contains("CANARY-SECRET"),
                        "identity field {key} contains a planted secret"
                    );
                }
                assert_identity_is_clean(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                assert_identity_is_clean(item);
            }
        }
        _ => {}
    }
}

fn assert_state_fixture(rel: &str) {
    let text = read_fixture(rel);
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("{rel} is not json"));
    assert_eq!(value["version"], 4, "{rel} is not state v4");
    for field in ["attributes", "sensitive_attributes", "private", "outputs"] {
        assert!(
            field_has_marker(&value, field),
            "{rel} has no planted secret under {field}"
        );
    }
    assert_identity_is_clean(&value);
}

fn canaries() -> Vec<String> {
    for rel in [OLD, NEW, BROKEN] {
        assert!(
            !rel.contains("CANARY-SECRET"),
            "fixture path must not itself be a planted secret"
        );
    }
    assert_state_fixture(OLD);
    assert_state_fixture(NEW);

    let broken = read_fixture(BROKEN);
    assert!(
        serde_json::from_str::<serde_json::Value>(&broken).is_err(),
        "broken fixture should not parse"
    );
    for field in ["attributes", "sensitive_attributes", "private", "outputs"] {
        assert!(broken.contains(field), "broken fixture missing {field}");
    }

    let mut tokens = BTreeSet::new();
    for rel in [OLD, NEW, BROKEN] {
        tokens.extend(planted_tokens(&read_fixture(rel)));
    }
    for required in REQUIRED {
        assert!(
            tokens.contains(*required),
            "fixtures are missing a required planted value"
        );
    }
    tokens.into_iter().collect()
}

fn check(args: &[&str], canaries: &[String], expect: Expect) {
    let output = run(args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let label = args.join(" ");
    assert_eq!(
        output.status.code(),
        Some(expect.code),
        "{label}: exit {} (stdout {} bytes, stderr {} bytes)",
        output.status.code().unwrap_or(-1),
        output.stdout.len(),
        output.stderr.len()
    );
    if expect.stdout_has.is_empty() {
        assert!(stdout.is_empty(), "{label}: stdout should be empty");
    }
    for needle in expect.stdout_has {
        assert!(stdout.contains(needle), "{label}: stdout missing {needle}");
    }
    if expect.stderr_has.is_empty() {
        assert!(stderr.is_empty(), "{label}: stderr should be empty");
    }
    for needle in expect.stderr_has {
        assert!(stderr.contains(needle), "{label}: stderr missing {needle}");
    }
    for canary in canaries {
        assert!(
            !stdout.contains(canary),
            "{label}: stdout contains a planted fixture secret"
        );
        assert!(
            !stderr.contains(canary),
            "{label}: stderr contains a planted fixture secret"
        );
    }
}

#[test]
fn output_contains_no_fixture_secrets() {
    let canaries = canaries();

    // The two states share random_id.kept[0], so count must not add them.
    let overlap = Expect {
        code: 2,
        stdout_has: &[],
        stderr_has: &["tests/fixtures/v4_canary_new.json: duplicate address random_id.kept[0]"],
    };
    check(&["count", OLD, NEW], &canaries, overlap);
    check(&["count", "--explain", OLD, NEW], &canaries, overlap);
    check(&["count", "--json", OLD, NEW], &canaries, overlap);
    check(
        &["count", "--json", "--explain", OLD, NEW],
        &canaries,
        overlap,
    );
    check(
        &["count", "--top", "1", "--explain", OLD, NEW],
        &canaries,
        overlap,
    );

    check(
        &["count", OLD],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &["total rum: 3\n", "estimate:"],
            stderr_has: &[],
        },
    );
    check(
        &["count", NEW],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &["total rum: 2\n", "estimate:"],
            stderr_has: &[],
        },
    );
    check(
        &["count", "--explain", NEW],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &[
                "random_id.kept[0] counts R1",
                "data.local_file.note does not count R2",
                "null_resource.wait[0] does not count R3",
                "random_id.absent does not count R4",
            ],
            stderr_has: &[],
        },
    );
    check(
        &["count", "--json", OLD],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &["\"schema_version\": 1", "\"total_rum\": 3"],
            stderr_has: &[],
        },
    );
    check(
        &["count", "--json", NEW],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &["\"schema_version\": 1", "\"total_rum\": 2"],
            stderr_has: &[],
        },
    );
    check(
        &["count", "--json", "--explain", NEW],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &[
                "\"schema_version\": 1",
                "\"address\": \"random_id.kept[0]\"",
            ],
            stderr_has: &[],
        },
    );
    check(
        &["count", "--top", "1", "--explain", OLD],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &["other:"],
            stderr_has: &[],
        },
    );
    check(
        &["count", "--max-rum", "0", OLD],
        &canaries,
        Expect {
            code: 1,
            stdout_has: &["total rum: 3\n"],
            stderr_has: &[],
        },
    );

    check(
        &["diff", OLD, NEW],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &[
                "rum: 3 -> 2\n",
                "rum delta: -1\n",
                "added:\n  random_id.kept[1]\n",
                "removed:\n  local_file.note\n  module.child.random_id.labeled[\"web\"]\n",
            ],
            stderr_has: &[],
        },
    );
    check(
        &["diff", "--explain", OLD, NEW],
        &canaries,
        Expect {
            code: 3,
            stdout_has: &[],
            stderr_has: &["--explain with diff only works together with --json"],
        },
    );
    check(
        &["diff", "--json", OLD, NEW],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &[
                "\"schema_version\": 1",
                "\"added\": [\n    \"random_id.kept[1]\"\n  ]",
                "\"removed\": [\n    \"local_file.note\",\n    \"module.child.random_id.labeled[\\\"web\\\"]\"\n  ]",
                "\"changed_rule\"",
                "\"rum_delta\": -1",
            ],
            stderr_has: &[],
        },
    );
    check(
        &["diff", "--json", "--explain", OLD, NEW],
        &canaries,
        Expect {
            code: 0,
            stdout_has: &["\"instances\"", "\"address\": \"local_file.note\""],
            stderr_has: &[],
        },
    );
    check(
        &["diff", "--max-increase", "0", NEW, OLD],
        &canaries,
        Expect {
            code: 1,
            stdout_has: &["rum delta: +1\n"],
            stderr_has: &[],
        },
    );

    let invalid = Expect {
        code: 2,
        stdout_has: &[],
        stderr_has: &["invalid json"],
    };
    check(&["count", BROKEN], &canaries, invalid);
    check(
        &["count", "--json", "--explain", BROKEN],
        &canaries,
        invalid,
    );
    check(&["count", OLD, BROKEN], &canaries, invalid);
    check(&["diff", BROKEN, NEW], &canaries, invalid);
    check(
        &["diff", "--json", "--explain", OLD, BROKEN],
        &canaries,
        invalid,
    );
}

fn lock_package_names(lock: &str) -> Vec<String> {
    lock.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("name = \"")?;
            let end = rest.find('"')?;
            Some(rest[..end].to_string())
        })
        .collect()
}

fn is_network_crate(name: &str) -> bool {
    NETWORK_ROOTS.iter().any(|root| {
        name == *root
            || name
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('-'))
    })
}

fn tree_crate_names(tree: &str) -> Vec<String> {
    tree.lines()
        .filter_map(|line| line.split_whitespace().next().map(str::to_string))
        .collect()
}

fn assert_src_has_no_std_net() {
    let src = manifest_path("src");
    let mut files = Vec::new();
    collect_rs(&src, &mut files);
    assert!(!files.is_empty(), "src has no rust files");
    for path in files {
        let text = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {}", path.display()));
        assert!(
            !text.contains("std::net"),
            "{} references std::net",
            path.display()
        );
    }
}

fn collect_rs(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|_| panic!("read dir {}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rs(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn dependencies_have_no_network_crates() {
    let lock = read_fixture("Cargo.lock");
    let names = lock_package_names(&lock);
    assert!(!names.is_empty(), "Cargo.lock has no package names");
    for name in &names {
        assert!(
            !is_network_crate(name),
            "Cargo.lock contains network crate {name}"
        );
    }

    let target = manifest_path("target/safety-tree");
    let output = Command::new("cargo")
        .args([
            "tree",
            "--offline",
            "--locked",
            "--prefix",
            "none",
            "--charset",
            "ascii",
            "--edges",
            "all",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_NET_OFFLINE", "true")
        .env_remove("CARGO_MAKEFLAGS")
        .output()
        .expect("cargo tree");
    assert!(
        output.status.success(),
        "cargo tree failed ({} bytes on stderr)",
        output.stderr.len()
    );
    let tree = String::from_utf8(output.stdout).expect("cargo tree is utf-8");
    let tree_names = tree_crate_names(&tree);
    assert!(!tree_names.is_empty(), "cargo tree printed no crates");
    for name in &tree_names {
        assert!(
            !is_network_crate(name),
            "cargo tree contains network crate {name}"
        );
    }

    assert_src_has_no_std_net();
}
