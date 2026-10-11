//! Every global option from `--help` must change `count` and `diff`, or exit 3.
//!
//! An option that only does something together with another flag exits 3
//! without that flag. A new global option without a case fails the walk.
//! The five refusal sentences are the same in README.md, docs/reference.md and `--help`.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};

const MAX_INCREASE_ONLY_DIFF: &str = "--max-increase only works with diff";
const MAX_RUM_ONLY_COUNT: &str = "--max-rum only works with count";
const HOURS_NEEDS_PRICES: &str = "--hours-per-month only works together with --prices";
const TOP_DIFF_NEEDS_JSON: &str = "--top with diff only works together with --json";
const EXPLAIN_DIFF_NEEDS_JSON: &str = "--explain with diff only works together with --json";

const MESSAGES: [&str; 5] = [
    MAX_INCREASE_ONLY_DIFF,
    MAX_RUM_ONLY_COUNT,
    HOURS_NEEDS_PRICES,
    TOP_DIFF_NEEDS_JSON,
    EXPLAIN_DIFF_NEEDS_JSON,
];

/// Clap's own flags, not fields of `Cli`. A new global option does not belong here.
const NOT_A_FIELD: &[&str] = &["--help", "--version"];

const COUNT_STATE: &str = "tests/fixtures/v4_top.json";
const DIFF_NEW: &str = "tests/fixtures/v4_diff_new.json";
const DIFF_OLD: &str = "tests/fixtures/v4_diff_old.json";
const PRICES: &str = "tests/fixtures/example-prices.json";
const MISSING: &str = "tests/fixtures/no-such-state.json";

const COUNT_FILES: &[&str] = &[COUNT_STATE];
/// New then old: the counting set grows by 2, so `--max-increase 1` exits 1.
const DIFF_FILES: &[&str] = &[DIFF_NEW, DIFF_OLD];

struct Case {
    count: Expect,
    diff: Expect,
}

enum Expect {
    /// Both calls exit 0 and the stdout differs. `with` sets the option, `baseline` does not.
    Output {
        baseline: &'static [&'static str],
        with: &'static [&'static str],
        files: &'static [&'static str],
    },
    /// `baseline` exits 0, `with` exits `code`.
    ExitCode {
        baseline: &'static [&'static str],
        with: &'static [&'static str],
        files: &'static [&'static str],
        code: i32,
    },
    /// Exit 3, empty stdout, stderr exactly `message`.
    Refuse {
        args: &'static [&'static str],
        message: &'static str,
        files: &'static [&'static str],
    },
    /// Without the other flag, exit 3. With it, stdout differs.
    Together {
        alone: &'static [&'static str],
        message: &'static str,
        baseline: &'static [&'static str],
        with: &'static [&'static str],
        files: &'static [&'static str],
    },
}

const CASES: &[(&str, Case)] = &[
    (
        "--json",
        Case {
            count: Expect::Output {
                baseline: &[],
                with: &["--json"],
                files: COUNT_FILES,
            },
            diff: Expect::Output {
                baseline: &[],
                with: &["--json"],
                files: DIFF_FILES,
            },
        },
    ),
    (
        "--explain",
        Case {
            count: Expect::Output {
                baseline: &[],
                with: &["--explain"],
                files: COUNT_FILES,
            },
            diff: Expect::Together {
                alone: &["--explain"],
                message: EXPLAIN_DIFF_NEEDS_JSON,
                baseline: &["--json"],
                with: &["--json", "--explain"],
                files: DIFF_FILES,
            },
        },
    ),
    (
        "--top",
        Case {
            count: Expect::Output {
                baseline: &[],
                with: &["--top", "1"],
                files: COUNT_FILES,
            },
            diff: Expect::Together {
                alone: &["--top", "1"],
                message: TOP_DIFF_NEEDS_JSON,
                baseline: &["--json"],
                with: &["--json", "--top", "1"],
                files: DIFF_FILES,
            },
        },
    ),
    (
        "--prices",
        Case {
            count: Expect::Output {
                baseline: &[],
                with: &["--prices", PRICES],
                files: COUNT_FILES,
            },
            diff: Expect::Output {
                baseline: &[],
                with: &["--prices", PRICES],
                files: DIFF_FILES,
            },
        },
    ),
    (
        "--hours-per-month",
        Case {
            count: Expect::Together {
                alone: &["--hours-per-month", "1"],
                message: HOURS_NEEDS_PRICES,
                baseline: &["--prices", PRICES],
                with: &["--prices", PRICES, "--hours-per-month", "1"],
                files: COUNT_FILES,
            },
            diff: Expect::Together {
                alone: &["--hours-per-month", "1"],
                message: HOURS_NEEDS_PRICES,
                baseline: &["--prices", PRICES],
                with: &["--prices", PRICES, "--hours-per-month", "1"],
                files: DIFF_FILES,
            },
        },
    ),
    (
        "--max-rum",
        Case {
            count: Expect::ExitCode {
                baseline: &[],
                with: &["--max-rum", "0"],
                files: COUNT_FILES,
                code: 1,
            },
            diff: Expect::Refuse {
                args: &["--max-rum", "0"],
                message: MAX_RUM_ONLY_COUNT,
                files: DIFF_FILES,
            },
        },
    ),
    (
        "--max-increase",
        Case {
            count: Expect::Refuse {
                args: &["--max-increase", "0"],
                message: MAX_INCREASE_ONLY_DIFF,
                files: COUNT_FILES,
            },
            diff: Expect::ExitCode {
                baseline: &[],
                with: &["--max-increase", "1"],
                files: DIFF_FILES,
                code: 1,
            },
        },
    ),
];

fn binary() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd
}

fn read_repo(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(sub: &str, flags: &[&str], files: &[&str]) -> Run {
    let mut args = Vec::with_capacity(1 + flags.len() + files.len());
    args.push(sub);
    args.extend(flags);
    args.extend(files);
    let output = binary().args(&args).output().expect("rum-meter binary");
    Run {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).expect("stdout is utf-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is utf-8"),
    }
}

fn has_canary(text: &str) -> bool {
    text.contains("CANARY")
}

fn no_canary(run: &Run) -> Option<String> {
    if has_canary(&run.stdout) || has_canary(&run.stderr) {
        Some(format!(
            "output contains a fixture marker (stdout {} bytes, stderr {} bytes)",
            run.stdout.len(),
            run.stderr.len()
        ))
    } else {
        None
    }
}

fn file_count(sub: &str) -> usize {
    match sub {
        "count" => 1,
        "diff" => 2,
        _ => panic!("unknown subcommand {sub}"),
    }
}

fn has_flag(flags: &[&str], option: &str) -> bool {
    flags.contains(&option)
}

fn other_options<'a>(flags: &'a [&str], option: &str) -> Vec<&'a str> {
    flags
        .iter()
        .copied()
        .filter(|part| part.starts_with("--") && *part != option)
        .collect()
}

fn reject_other_options(option: &str, flags: &[&str]) -> Option<String> {
    let others = other_options(flags, option);
    if others.is_empty() {
        None
    } else {
        Some(format!(
            "{option} is combined with {others:?}; without that option the case must exit 3"
        ))
    }
}

fn value_after<'a>(flags: &'a [&str], option: &str) -> Option<&'a str> {
    let pos = flags.iter().position(|part| *part == option)?;
    flags
        .get(pos + 1)
        .copied()
        .filter(|value| !value.starts_with("--"))
}

fn clap_default<'a>(help: &'a str, option: &str) -> Option<&'a str> {
    for line in help.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('-') {
            continue;
        }
        if !trimmed.split_whitespace().any(|word| word == option) {
            continue;
        }
        let marker = "[default: ";
        let start = line.find(marker)? + marker.len();
        let rest = &line[start..];
        let end = rest.find(']')?;
        return Some(rest[..end].trim());
    }
    None
}

fn reject_default(help: &str, option: &str, flags: &[&str]) -> Option<String> {
    let default = clap_default(help, option)?;
    match value_after(flags, option) {
        Some(value) if value != default => None,
        Some(value) => Some(format!(
            "{option} {value} is the default; another value must change the output"
        )),
        None => Some(format!(
            "{option} defaults to {default}; the case must set another value"
        )),
    }
}

fn files_of(expect: &Expect) -> &'static [&'static str] {
    match expect {
        Expect::Output { files, .. }
        | Expect::ExitCode { files, .. }
        | Expect::Refuse { files, .. }
        | Expect::Together { files, .. } => files,
    }
}

fn check(help: &str, sub: &str, option: &str, expect: &Expect) -> Option<String> {
    let files = files_of(expect);
    if files.len() != file_count(sub) {
        return Some(format!(
            "{sub} expected {} files, got {}",
            file_count(sub),
            files.len()
        ));
    }
    match expect {
        Expect::Output {
            baseline,
            with,
            files,
        } => check_output(help, sub, option, baseline, with, files),
        Expect::ExitCode {
            baseline,
            with,
            files,
            code,
        } => check_exit(help, sub, option, baseline, with, files, *code),
        Expect::Refuse {
            args,
            message,
            files,
        } => check_refuse(help, sub, option, args, message, files),
        Expect::Together {
            alone,
            message,
            baseline,
            with,
            files,
        } => check_together(
            help,
            sub,
            option,
            TogetherArgs {
                alone,
                message,
                baseline,
                with,
                files,
            },
        ),
    }
}

fn check_output(
    help: &str,
    sub: &str,
    option: &str,
    baseline: &[&str],
    with: &[&str],
    files: &[&str],
) -> Option<String> {
    if !has_flag(with, option) || has_flag(baseline, option) {
        return Some(format!("{option} belongs only on the call that sets it"));
    }
    if let Some(err) = reject_other_options(option, with) {
        return Some(err);
    }
    if let Some(err) = reject_default(help, option, with) {
        return Some(err);
    }
    let left = run(sub, baseline, files);
    let right = run(sub, with, files);
    if let Some(err) = no_canary(&left).or_else(|| no_canary(&right)) {
        return Some(err);
    }
    if left.code != Some(0) || right.code != Some(0) || left.stdout == right.stdout {
        Some(format!(
            "no effect on the output: baseline status={:?} ({} bytes) with status={:?} ({} bytes)",
            left.code,
            left.stdout.len(),
            right.code,
            right.stdout.len()
        ))
    } else {
        None
    }
}

fn check_exit(
    help: &str,
    sub: &str,
    option: &str,
    baseline: &[&str],
    with: &[&str],
    files: &[&str],
    code: i32,
) -> Option<String> {
    if code == 0 {
        return Some(format!("{option}: exit 0 does not count as an effect"));
    }
    if !has_flag(with, option) || has_flag(baseline, option) {
        return Some(format!("{option} belongs only on the call that sets it"));
    }
    if let Some(err) = reject_other_options(option, with) {
        return Some(err);
    }
    if let Some(err) = reject_default(help, option, with) {
        return Some(err);
    }
    let left = run(sub, baseline, files);
    let right = run(sub, with, files);
    if let Some(err) = no_canary(&left).or_else(|| no_canary(&right)) {
        return Some(err);
    }
    if left.code != Some(0) || right.code != Some(code) {
        Some(format!(
            "exit code: baseline {:?} with {:?} (expected {code}), stderr {} bytes",
            left.code,
            right.code,
            right.stderr.len()
        ))
    } else {
        None
    }
}

fn check_refuse(
    help: &str,
    sub: &str,
    option: &str,
    args: &[&str],
    message: &str,
    files: &[&str],
) -> Option<String> {
    if !has_flag(args, option) {
        return Some(format!("{option} is missing from the refusal"));
    }
    if let Some(err) = reject_other_options(option, args) {
        return Some(err);
    }
    if let Some(err) = reject_default(help, option, args) {
        return Some(err);
    }
    let out = run(sub, args, files);
    if let Some(err) = no_canary(&out) {
        return Some(err);
    }
    let expected = format!("{message}\n");
    if out.code != Some(3) || !out.stdout.is_empty() || out.stderr != expected {
        Some(format!(
            "refusal: status={:?} stdout {} bytes stderr {} bytes",
            out.code,
            out.stdout.len(),
            out.stderr.len()
        ))
    } else {
        None
    }
}

struct TogetherArgs<'a> {
    alone: &'a [&'a str],
    message: &'a str,
    baseline: &'a [&'a str],
    with: &'a [&'a str],
    files: &'a [&'a str],
}

fn check_together(help: &str, sub: &str, option: &str, case: TogetherArgs<'_>) -> Option<String> {
    let TogetherArgs {
        alone,
        message,
        baseline,
        with,
        files,
    } = case;
    if let Some(err) = reject_other_options(option, alone) {
        return Some(err);
    }
    if !has_flag(alone, option) {
        return Some(format!("{option} is missing from the alone call"));
    }
    if other_options(with, option).is_empty()
        || !has_flag(with, option)
        || has_flag(baseline, option)
    {
        return Some(format!(
            "{option}: the combined call must set the option together with another option"
        ));
    }
    if let Some(err) = reject_default(help, option, with) {
        return Some(err);
    }
    let out = run(sub, alone, files);
    if let Some(err) = no_canary(&out) {
        return Some(err);
    }
    let expected = format!("{message}\n");
    if out.code != Some(3) || !out.stdout.is_empty() || out.stderr != expected {
        return Some(format!(
            "without the combination: status={:?} stdout {} bytes stderr {} bytes",
            out.code,
            out.stdout.len(),
            out.stderr.len()
        ));
    }
    let left = run(sub, baseline, files);
    let right = run(sub, with, files);
    if let Some(err) = no_canary(&left).or_else(|| no_canary(&right)) {
        return Some(err);
    }
    if left.code != Some(0) || right.code != Some(0) || left.stdout == right.stdout {
        Some(format!(
            "together, no effect: baseline status={:?} ({} bytes) with status={:?} ({} bytes)",
            left.code,
            left.stdout.len(),
            right.code,
            right.stdout.len()
        ))
    } else {
        None
    }
}

fn global_options(help: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_options = false;
    for line in help.lines() {
        if !in_options {
            if line == "Options:" {
                in_options = true;
            }
            continue;
        }
        if line.is_empty() {
            break;
        }
        let trimmed = line.trim_start();
        if !trimmed.starts_with('-') {
            continue;
        }
        for word in trimmed.split_whitespace() {
            let Some(rest) = word.strip_prefix("--") else {
                continue;
            };
            if rest.is_empty()
                || !rest
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                continue;
            }
            let name = format!("--{rest}");
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    assert!(
        !names.is_empty(),
        "no options in --help ({} bytes)",
        help.len()
    );
    names
}

fn help_of(args: &[&str]) -> String {
    let output = binary().args(args).output().expect("rum-meter binary");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{args:?} help exit {:?}",
        output.status.code()
    );
    String::from_utf8(output.stdout).expect("help is utf-8")
}

fn assert_refusal(args: &[&str], message: &str) {
    let output = binary().args(args).output().expect("rum-meter binary");
    assert_exact_refusal(&output, args, message);
}

fn assert_exact_refusal(output: &Output, args: &[&str], message: &str) {
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    let leaked = output
        .stdout
        .windows(b"CANARY".len())
        .any(|window| window == b"CANARY")
        || stderr.contains("CANARY");
    assert!(
        output.status.code() == Some(3)
            && output.stdout.is_empty()
            && stderr == format!("{message}\n")
            && !leaked,
        "{args:?}: exit {:?} stdout bytes {} stderr bytes {} leaked {leaked}",
        output.status.code(),
        output.stdout.len(),
        stderr.len()
    );
}

#[test]
fn max_increase_on_count_exits_3() {
    let one = "tests/fixtures/v4_one_managed.json";
    assert_refusal(
        &["count", "--max-increase", "0", one],
        MAX_INCREASE_ONLY_DIFF,
    );
    assert_refusal(
        &["count", "--json", "--explain", "--max-increase", "4", one],
        MAX_INCREASE_ONLY_DIFF,
    );
    assert_refusal(
        &["count", "--max-increase", "0", MISSING],
        MAX_INCREASE_ONLY_DIFF,
    );
}

#[test]
fn max_rum_on_diff_exits_3() {
    assert_refusal(
        &["diff", "--max-rum", "0", DIFF_OLD, DIFF_NEW],
        MAX_RUM_ONLY_COUNT,
    );
    assert_refusal(
        &["diff", "--json", "--max-rum", "1", DIFF_NEW, DIFF_OLD],
        MAX_RUM_ONLY_COUNT,
    );
    assert_refusal(
        &["diff", "--max-rum", "0", MISSING, MISSING],
        MAX_RUM_ONLY_COUNT,
    );
}

#[test]
fn hours_without_prices_exits_3() {
    let one = "tests/fixtures/v4_one_managed.json";
    for args in [
        &["count", "--hours-per-month", "10", one][..],
        &["count", "--json", "--hours-per-month", "10", one][..],
        &["diff", "--hours-per-month", "10", DIFF_OLD, DIFF_NEW][..],
        &[
            "diff",
            "--json",
            "--hours-per-month",
            "10",
            DIFF_OLD,
            DIFF_NEW,
        ][..],
        &["count", "--hours-per-month", "10", MISSING][..],
        &["diff", "--json", "--hours-per-month", "1", MISSING, MISSING][..],
    ] {
        assert_refusal(args, HOURS_NEEDS_PRICES);
    }
}

#[test]
fn top_on_diff_without_json_exits_3() {
    assert_refusal(
        &["diff", "--top", "1", DIFF_OLD, DIFF_NEW],
        TOP_DIFF_NEEDS_JSON,
    );
    assert_refusal(
        &["diff", "--top", "3", MISSING, MISSING],
        TOP_DIFF_NEEDS_JSON,
    );
}

#[test]
fn explain_on_diff_without_json_exits_3() {
    assert_refusal(
        &["diff", "--explain", DIFF_OLD, DIFF_NEW],
        EXPLAIN_DIFF_NEEDS_JSON,
    );
    assert_refusal(
        &["diff", "--explain", MISSING, MISSING],
        EXPLAIN_DIFF_NEEDS_JSON,
    );
}

#[test]
fn first_refusal_wins_when_several_are_set() {
    assert_refusal(
        &[
            "count",
            "--explain",
            "--hours-per-month",
            "1",
            "--max-increase",
            "0",
            MISSING,
        ],
        MAX_INCREASE_ONLY_DIFF,
    );
    assert_refusal(
        &[
            "diff",
            "--explain",
            "--top",
            "1",
            "--hours-per-month",
            "1",
            "--max-rum",
            "0",
            MISSING,
            MISSING,
        ],
        MAX_RUM_ONLY_COUNT,
    );
    assert_refusal(
        &[
            "diff",
            "--explain",
            "--top",
            "1",
            "--json",
            "--hours-per-month",
            "1",
            MISSING,
            MISSING,
        ],
        HOURS_NEEDS_PRICES,
    );
    assert_refusal(
        &["diff", "--explain", "--top", "1", MISSING, MISSING],
        TOP_DIFF_NEEDS_JSON,
    );
}

#[test]
fn every_global_option_has_effect_or_exits_3() {
    let help = help_of(&["--help"]);
    let names = global_options(&help);
    let from_help: BTreeSet<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| !NOT_A_FIELD.contains(name))
        .collect();
    let mut seen = BTreeSet::new();
    for (name, _) in CASES {
        assert!(seen.insert(*name), "duplicate case {name}");
    }
    let from_cases: BTreeSet<&str> = seen;

    let mut errors = Vec::new();
    for name in &from_help {
        if !from_cases.contains(name) {
            errors.push(format!("no case for global option {name}"));
        }
    }
    for name in &from_cases {
        if !from_help.contains(name) {
            errors.push(format!("case {name} is not a global option in --help"));
        }
    }

    for (option, case) in CASES {
        for (sub, expect) in [("count", &case.count), ("diff", &case.diff)] {
            if let Some(err) = check(&help, sub, option, expect) {
                errors.push(format!("{sub} {option}: {err}"));
            }
        }
    }
    assert!(
        errors.is_empty(),
        "global options with no effect and no exit 3:\n{}",
        errors.join("\n")
    );
}

#[test]
fn refusal_messages_match_in_readme_help_and_reference() {
    let readme = read_repo("README.md");
    let reference = read_repo("docs/reference.md");
    for message in MESSAGES {
        assert!(readme.contains(message), "README is missing: {message}");
        assert!(
            reference.contains(message),
            "reference is missing: {message}"
        );
    }
    for args in [&["--help"][..], &["count", "--help"], &["diff", "--help"]] {
        let help = help_of(args);
        for message in MESSAGES {
            assert!(help.contains(message), "{args:?} is missing: {message}");
        }
    }
}
