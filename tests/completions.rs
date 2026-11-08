//! `rum-meter completions` writes a bash or zsh script, and stays out of `--help`.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    let path = option_env!("CARGO_BIN_EXE_rum_meter").unwrap_or(env!("CARGO_BIN_EXE_rum-meter"));
    Command::new(path)
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("start rum-meter")
}

#[test]
fn bash_script_names_the_options() {
    let out = run(&["completions", "bash"]);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("complete "));
    assert!(text.contains("--max-rum"));
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
