//! The files in `examples/` are fictional states. Their expected output is kept
//! next to them, and the binary must print it byte for byte with exit 0 and an
//! empty stderr.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn golden(args: &[&str], expected_file: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .current_dir(&root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("start rum-meter {args:?}: {err}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{args:?} stderr: {stderr}");
    assert!(stderr.is_empty(), "{args:?} stderr: {stderr}");
    let expected = fs::read_to_string(root.join(expected_file))
        .unwrap_or_else(|err| panic!("read {expected_file}: {err}"));
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout is utf-8"),
        expected,
        "{args:?} differs from {expected_file}"
    );
}

#[test]
fn count_explain_matches_the_golden_file() {
    golden(
        &["count", "--explain", "examples/web-stack.tfstate"],
        "examples/web-stack.count.txt",
    );
}

#[test]
fn diff_matches_the_golden_file() {
    golden(
        &[
            "diff",
            "examples/web-stack.tfstate",
            "examples/web-stack-next.tfstate",
        ],
        "examples/web-stack.diff.txt",
    );
}

#[test]
fn single_resource_example_matches_the_documented_output() {
    golden(
        &["count", "examples/one-managed.tfstate"],
        "examples/one-managed.count.txt",
    );
}

#[test]
fn examples_hold_no_secrets() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    for entry in fs::read_dir(dir).expect("read examples") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|ext| ext == "md") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_default().to_lowercase();
        for word in ["secret", "token", "password", "private_key", "begin "] {
            assert!(!text.contains(word), "{} contains {word}", path.display());
        }
    }
}
