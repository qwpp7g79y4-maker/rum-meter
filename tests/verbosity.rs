//! `--quiet` and `--verbose`. A pipe prints no hint and no context line.
//! `--verbose` adds that context. Exit codes stay 0, 1, 2 and 3.

use std::process::Command;

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("binary start");
    (
        output.status.code(),
        String::from_utf8(output.stdout).expect("stdout is utf-8"),
        String::from_utf8(output.stderr).expect("stderr is utf-8"),
    )
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn help_lists_quiet_and_verbose() {
    for args in [&["--help"][..], &["count", "--help"], &["diff", "--help"]] {
        let (code, stdout, stderr) = run(args);
        assert_eq!(code, Some(0), "{args:?} {stderr}");
        assert!(stdout.contains("--quiet"), "{args:?}\n{stdout}");
        assert!(stdout.contains("--verbose"), "{args:?}\n{stdout}");
    }
}

#[test]
fn pipe_and_quiet_omit_the_hint_verbose_prints_it() {
    let missing = "/nonexistent/rum-meter-verbosity.json";
    let (code, stdout, stderr) = run(&["count", missing]);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    assert!(stderr.contains("unreadable"), "{stderr}");
    assert!(!stderr.contains("hint:"), "{stderr}");

    let (qcode, qout, qerr) = run(&["--quiet", "count", missing]);
    assert_eq!(qcode, Some(2), "{qerr}");
    assert!(qout.is_empty());
    assert_eq!(qerr, stderr, "quiet matches a pipe");

    let (vcode, vout, verr) = run(&["--verbose", "count", missing]);
    assert_eq!(vcode, Some(2), "{verr}");
    assert!(vout.is_empty());
    assert_eq!(verr.lines().count(), 2, "{verr:?}");
    assert!(verr.lines().next().unwrap().contains("unreadable"));
    assert!(verr.lines().nth(1).unwrap().starts_with("hint: "), "{verr}");
}

#[test]
fn quiet_with_verbose_is_exit_3() {
    let file = fixture("v4_one_managed.json");
    let (code, stdout, stderr) = run(&["count", "--quiet", "--verbose", &file]);
    assert_eq!(code, Some(3), "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("error:"), "{stderr}");
    assert_ne!(code, Some(1), "no new threshold exit");
}

#[test]
fn verbose_context_stays_off_stdout() {
    let file = fixture("v4_one_managed.json");
    let (code, stdout, stderr) = run(&["count", &file]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(stdout.contains("total rum: 1\n"), "{stdout}");
    let (vcode, vout, verr) = run(&["count", "--verbose", &file]);
    assert_eq!(vcode, Some(0), "{verr}");
    assert_eq!(vout, stdout, "verbose does not change the report");
    assert_eq!(verr, "context: states 1, rum 1\n", "{verr:?}");
    let (qcode, qout, qerr) = run(&["count", "--quiet", &file]);
    assert_eq!(qcode, Some(0), "{qerr}");
    assert_eq!(qout, stdout);
    assert!(qerr.is_empty(), "{qerr}");
}

#[test]
fn diff_verbose_names_both_sides() {
    let old = fixture("v4_diff_old.json");
    let new = fixture("v4_diff_new.json");
    let (code, stdout, stderr) = run(&["diff", &old, &new]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    let rum = stdout
        .lines()
        .find(|line| line.starts_with("rum: "))
        .expect("rum line")
        .trim_start_matches("rum: ");
    let (vcode, vout, verr) = run(&["diff", "--verbose", &old, &new]);
    assert_eq!(vcode, Some(0), "{verr}");
    assert_eq!(vout, stdout);
    assert_eq!(verr, format!("context: rum {rum}\n"), "{verr:?}");
    assert!(verr.contains("->"), "{verr:?}");
    assert_eq!(verr.lines().count(), 1, "{verr:?}");
}

#[test]
fn threshold_exit_stays_1_and_the_pipe_is_silent() {
    let file = fixture("v4_one_managed.json");
    let (code, stdout, stderr) = run(&["count", "--max-rum", "0", &file]);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(stdout.contains("total rum: 1\n"), "{stdout}");
    assert!(stderr.is_empty(), "{stderr:?}");

    let (qcode, qout, qerr) = run(&["--quiet", "count", "--max-rum", "0", &file]);
    assert_eq!(qcode, Some(1), "{qerr}");
    assert_eq!(qout, stdout);
    assert_eq!(qerr, stderr);

    let (vcode, vout, verr) = run(&["--verbose", "count", "--max-rum", "0", &file]);
    assert_eq!(vcode, Some(1), "{verr}");
    assert_eq!(vout, stdout, "the threshold still prints the report");
    assert_eq!(
        verr, "hint: the summed RUM is above --max-rum; the report is unchanged\n",
        "{verr:?}"
    );
    assert_ne!(vcode, Some(2));
}

#[test]
fn diff_rejection_stays_one_line_without_a_new_exit() {
    let old = fixture("v4_diff_old.json");
    let new = fixture("v4_diff_new.json");
    let (code, stdout, stderr) = run(&["diff", "--explain", &old, &new]);
    assert_eq!(code, Some(3), "{stderr}");
    assert!(stdout.is_empty());
    assert_eq!(
        stderr, "--explain with diff only works together with --json\n",
        "{stderr:?}"
    );
    let (vcode, vout, verr) = run(&["diff", "--verbose", "--explain", &old, &new]);
    assert_eq!(vcode, Some(3), "{verr}");
    assert!(vout.is_empty());
    assert_eq!(
        verr, stderr,
        "verbose does not renumber or extend this line"
    );
    assert_ne!(vcode, Some(1));
}
