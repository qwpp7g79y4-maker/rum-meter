//! `rum-meter completions` writes a bash or zsh script, and stays out of `--help`.

use std::collections::BTreeSet;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    let path = option_env!("CARGO_BIN_EXE_rum_meter").unwrap_or(env!("CARGO_BIN_EXE_rum-meter"));
    Command::new(path)
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("start rum-meter")
}

/// Long option names from `--help` or from a completion script, without `--`.
fn long_options(text: &str) -> BTreeSet<String> {
    text.split_whitespace()
        .filter_map(|word| word.strip_prefix("--"))
        .map(|name| {
            name.chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

#[test]
fn bash_script_names_the_options() {
    let out = run(&["completions", "bash"]);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("complete "));
    assert!(text.contains("--max-rum"));
    let help_out = run(&["--help"]);
    assert_eq!(help_out.status.code(), Some(0));
    let help = String::from_utf8(help_out.stdout).unwrap();
    let expected = long_options(&help);
    assert!(
        expected.contains("help") && expected.contains("version"),
        "top-level --help should name --help and --version: {expected:?}"
    );
    let in_script = long_options(&text);
    for option in &expected {
        assert!(
            in_script.contains(option),
            "bash completion lacks --{option}"
        );
    }
}

#[test]
fn zsh_script_starts_with_compdef() {
    let out = run(&["completions", "zsh"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .starts_with("#compdef rum-meter")
    );
}

#[test]
fn other_shell_is_a_usage_error() {
    let out = run(&["completions", "fish"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
}

#[test]
fn help_does_not_list_completions() {
    let out = run(&["--help"]);
    assert!(
        !String::from_utf8(out.stdout)
            .unwrap()
            .contains("completions")
    );
}
