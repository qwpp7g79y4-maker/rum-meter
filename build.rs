//! Debug builds append the commit `git rev-parse --verify HEAD` prints.
//! When git does not print one, the version stays the crate version.
//! Release builds do not include a sha.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    watch_git_head();

    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let debug_version = match git_sha() {
        Some(sha) => format!("{version} ({sha})"),
        None => version.clone(),
    };
    let source = format!(
        "#[cfg(debug_assertions)]\n\
         pub const DISPLAY_VERSION: &str = \"{debug_version}\";\n\
         #[cfg(not(debug_assertions))]\n\
         pub const DISPLAY_VERSION: &str = \"{version}\";\n"
    );
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("display_version.rs");
    std::fs::write(&dest, source).expect("write display version");
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn git_sha() -> Option<String> {
    let sha = git_output(&["rev-parse", "--verify", "HEAD"])?;
    if sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(sha)
    } else {
        None
    }
}

fn watch_git_head() {
    let Some(head) = git_output(&["rev-parse", "--git-path", "HEAD"]) else {
        return;
    };
    let head_path = resolve_git_path(&head);
    println!("cargo:rerun-if-changed={}", head_path.display());
    let Ok(contents) = std::fs::read_to_string(&head_path) else {
        return;
    };
    let Some(rest) = contents.trim().strip_prefix("ref: ") else {
        return;
    };
    if !rest.starts_with("refs/") || rest.contains(['\n', '\r']) {
        return;
    }
    if let Some(ref_path) = git_output(&["rev-parse", "--git-path", rest]) {
        println!(
            "cargo:rerun-if-changed={}",
            resolve_git_path(&ref_path).display()
        );
    }
    if let Some(packed) = git_output(&["rev-parse", "--git-path", "packed-refs"]) {
        println!(
            "cargo:rerun-if-changed={}",
            resolve_git_path(&packed).display()
        );
    }
}

fn resolve_git_path(path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        manifest_dir().join(candidate)
    }
}
