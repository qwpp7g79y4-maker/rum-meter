//! The manual page names the same long options as `rum-meter --help`.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn binary() -> Command {
    // This Cargo exports `CARGO_BIN_EXE_rum-meter` (hyphen), which is the name
    // in its own docs. Prefer the underscore spelling when a toolchain sets it.
    let path = option_env!("CARGO_BIN_EXE_rum_meter").unwrap_or(env!("CARGO_BIN_EXE_rum-meter"));
    let mut cmd = Command::new(path);
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd
}

fn manifest(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn output_of(args: &[&str]) -> Output {
    binary()
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("start rum-meter {args:?}: {err}"))
}

/// Long options are `--` plus a name of lowercase ASCII letters, digits and hyphens.
fn long_options(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            let start = i;
            i += 2;
            let name_start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_lowercase() || bytes[i].is_ascii_digit() || bytes[i] == b'-')
            {
                i += 1;
            }
            let name = &text[name_start..i];
            if !name.is_empty() && !name.starts_with('-') && !name.ends_with('-') {
                found.insert(text[start..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    found
}

fn subcommand_names(help: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_commands = false;
    for line in help.lines() {
        if !in_commands {
            if line == "Commands:" {
                in_commands = true;
            }
            continue;
        }
        if line.is_empty() {
            break;
        }
        let name = line
            .split_whitespace()
            .next()
            .unwrap_or_else(|| panic!("command line has no name: {line}"));
        names.push(name.to_string());
    }
    assert!(!names.is_empty(), "no commands in --help");
    names
}

fn assert_no_panic(output: &Output) {
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        !err.contains("panicked"),
        "stderr contains panicked:\n{err}"
    );
}

#[test]
fn help_long_options_match_the_manual_page() {
    let output = output_of(&["--help"]);
    assert!(
        output.status.success(),
        "--help should exit 0, got {:?}",
        output.status.code()
    );
    assert_no_panic(&output);
    let help = String::from_utf8(output.stdout).expect("help is utf-8");
    let man = fs::read_to_string(manifest("man/rum-meter.1")).expect("read man page");
    for heading in [
        ".SH NAME",
        ".SH SYNOPSIS",
        ".SH DESCRIPTION",
        ".SH OPTIONS",
        ".SH \"EXIT STATUS\"",
        ".SH EXAMPLES",
    ] {
        assert!(man.contains(heading), "man page is missing {heading}");
    }

    let from_help = long_options(&help);
    let from_man = long_options(&man);
    assert!(
        from_help.contains("--help") && from_help.contains("--version"),
        "top-level help should show --help and --version: {from_help:?}"
    );
    assert_eq!(
        from_man, from_help,
        "man page long options differ from --help"
    );

    let names = subcommand_names(&help);
    assert!(
        man.contains("count") && man.contains("diff") && man.contains("help"),
        "man page should name every subcommand from --help: {names:?}"
    );
    for name in &names {
        assert!(
            man.contains(name),
            "man page does not name subcommand {name}"
        );
        if name == "help" {
            let refused = output_of(&["help", "--help"]);
            assert!(!refused.status.success(), "help --help should not exit 0");
            assert_no_panic(&refused);
            let own = output_of(&["help", "help"]);
            assert!(own.status.success(), "help help should exit 0");
            assert_no_panic(&own);
            let own_help = String::from_utf8(own.stdout).expect("help help is utf-8");
            assert!(
                long_options(&own_help).is_subset(&from_help),
                "help help has a long option that top-level --help does not"
            );
            continue;
        }
        let sub = output_of(&[name, "--help"]);
        assert!(
            sub.status.success(),
            "{name} --help should exit 0, got {:?}",
            sub.status.code()
        );
        assert_no_panic(&sub);
        let sub_help = String::from_utf8(sub.stdout).expect("subcommand help is utf-8");
        let sub_opts = long_options(&sub_help);
        assert!(
            sub_opts.is_subset(&from_help),
            "{name} --help has a long option that top-level --help does not: {sub_opts:?}"
        );
        for opt in &sub_opts {
            assert!(
                man.contains(opt),
                "man page is missing {opt} from {name} --help"
            );
        }
    }
}

#[test]
fn unknown_option_does_not_panic() {
    let output = output_of(&["--not-a-handbook-option"]);
    assert!(!output.status.success(), "unknown option should not exit 0");
    assert_no_panic(&output);
}

#[test]
fn example_command_exits_as_documented() {
    let text = fs::read_to_string(manifest("examples/README.md")).expect("read examples readme");
    let mut commands = Vec::new();
    let mut exits = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line == "```text" {
            let cmd = lines
                .next()
                .unwrap_or_else(|| panic!("example fence has no command"));
            assert_eq!(
                lines.next(),
                Some("```"),
                "example command fence is not one line: {cmd}"
            );
            let rest = cmd
                .strip_prefix("rum-meter ")
                .unwrap_or_else(|| panic!("command does not start with rum-meter: {cmd}"));
            commands.push(
                rest.split_whitespace()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            );
        }
        if let Some(rest) = line.strip_prefix("Expected exit code: ") {
            exits.push(rest.trim().parse::<i32>().expect("exit code number"));
        }
    }
    assert_eq!(
        commands.len(),
        1,
        "examples/README.md should document one command"
    );
    assert_eq!(
        exits.len(),
        1,
        "examples/README.md should document one exit code"
    );
    let args = &commands[0];
    let want = exits[0];
    let state = manifest("examples/one-managed.tfstate");
    assert!(state.is_file(), "missing {}", state.display());
    let output = binary().args(args).output().expect("start example command");
    assert_eq!(
        output.status.code(),
        Some(want),
        "example {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_no_panic(&output);
}
