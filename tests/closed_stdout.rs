//! A stdout that is closed at start (`>&-`) is exit 2 with a message.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_rum-meter");
const STATE: &str = "tests/fixtures/v4_one_managed.json";

#[cfg(unix)]
fn closed(args: &str) -> std::process::Output {
    Command::new("sh")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["-c", &format!("exec \"{BIN}\" {args} >&-")])
        .output()
        .unwrap()
}

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "macos"
    )
))]
#[test]
fn closed_stdout_is_exit_2_with_a_message() {
    let out = closed(&format!("count {STATE}"));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "rum-meter: cannot write report to stdout; check that stdout is open and the disk is not full\n"
    );
    for args in ["--version", "--help", "completions bash"] {
        let out = closed(args);
        assert_eq!(out.status.code(), Some(2), "{args}");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            "rum-meter: cannot write message to stdout\n",
            "{args}"
        );
    }
}

#[cfg(unix)]
#[test]
fn dev_null_stdout_is_still_exit_0() {
    let out = Command::new(BIN)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["count", STATE])
        .stdout(
            std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .unwrap(),
        )
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
}
