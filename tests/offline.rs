//! The count is the same when the process has no way off the machine.
//!
//! W-45. `unshare --user --map-root-user --net` starts a network namespace
//! whose route table is empty, so nothing can connect out. The count reads
//! the existing example state. No price file is passed. `--output` is unchanged.

#![cfg(target_os = "linux")]

use std::fs;
use std::process::Command;

/// The one-managed count, with outgoing connections blocked.
///
/// The proof reads `/proc/net/route` in that namespace. `count` then reads
/// `examples/one-managed.tfstate` and must print `examples/one-managed.count.txt`.
/// No price file is passed.
#[test]
fn one_managed_count_matches_expected_output_with_outgoing_connections_blocked() {
    // GitHub CI / pepai-worker-01 often lack user namespaces (EPERM / uid_map).
    let probe = Command::new("unshare")
        .args(["-rn", "true"])
        .output()
        .expect("unshare -rn true");
    if !probe.status.success() {
        let err = String::from_utf8_lossy(&probe.stderr);
        let low = err.to_ascii_lowercase();
        if low.contains("operation not permitted")
            || low.contains("eperm")
            || low.contains("uid_map")
            || low.contains("permission denied")
        {
            eprintln!("overgeslagen: geen user namespace");
            return;
        }
        panic!("unshare -rn true failed: {err}");
    }

    let ns = ["--user", "--map-root-user", "--net", "--"];
    let proof = Command::new("unshare")
        .args(ns)
        .args(["cat", "/proc/net/route"])
        .output()
        .expect("unshare route table");
    assert!(
        proof.status.success(),
        "unshare failed: {}",
        String::from_utf8_lossy(&proof.stderr)
    );
    let routes = String::from_utf8(proof.stdout).expect("utf-8 route table");
    assert!(
        !routes.split_whitespace().any(|field| field == "00000000"),
        "network namespace still has a route: {routes:?}"
    );

    let root = env!("CARGO_MANIFEST_DIR");
    let out = Command::new("unshare")
        .args(ns)
        .arg(env!("CARGO_BIN_EXE_rum-meter"))
        .current_dir(root)
        .args(["count", "examples/one-managed.tfstate"])
        .output()
        .expect("run rum-meter with outgoing connections blocked");
    let want = fs::read_to_string(format!("{root}/examples/one-managed.count.txt"))
        .expect("expected file");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).expect("utf-8"), want);
}
