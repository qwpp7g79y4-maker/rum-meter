//! Fixture `v4_top.json` is handwritten. Billable counts:
//! modules: compute 5, network 5, app 3, data 2, root 1 (16).
//! types: local_file 4, random_id 4, random_integer 3, random_pet 3, random_string 2 (16).
//! A data source, null_resource, terraform_data and an empty resource are in the file and do not count.
//! `--top 3` keeps the first three rows; module rest is 2+1 and type rest is 3+2.

use std::process::{Command, Output};

const ESTIMATE: &str = "estimate: HCP Terraform bills the hourly peak; this is a single snapshot";
const FIXTURE: &str = "tests/fixtures/v4_top.json";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary")
}

fn group<'a>(stdout: &'a str, header: &str) -> Vec<&'a str> {
    let mut lines = stdout.lines();
    let wanted = format!("{header}:");
    assert!(lines.any(|line| line == wanted), "missing {header}");
    lines
        .take_while(|line| line.starts_with("  "))
        .map(str::trim_start)
        .collect()
}

fn row_count(row: &str) -> usize {
    row.rsplit_once(": ")
        .expect("name and count")
        .1
        .parse()
        .expect("count")
}

fn other_count(rows: &[&str]) -> usize {
    let row = rows
        .iter()
        .find(|row| row.starts_with("other: "))
        .expect("other row");
    row_count(row)
}

fn assert_count_ok(output: &Output) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is utf-8");
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert_eq!(output.status.code(), Some(0), "stderr:\n{stderr}");
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("CANARY-SECRET"),
        "stdout leaked a fixture value"
    );
    assert!(stdout.ends_with(&format!("{ESTIMATE}\n")));
    stdout
}

#[test]
fn fixture_lists_four_modules_and_five_types_in_order() {
    let stdout = assert_count_ok(&run(&["count", FIXTURE]));
    assert!(stdout.lines().any(|line| line == "total rum: 16"));
    assert!(
        stdout
            .lines()
            .any(|line| line == "not counted: R2 1, R3 3, R4 1")
    );
    let modules = group(&stdout, "modules");
    let types = group(&stdout, "types");
    assert_eq!(
        modules,
        [
            "module.compute: 5",
            "module.network: 5",
            "module.app: 3",
            "module.data: 2",
            "(root): 1",
        ]
    );
    assert_eq!(
        types,
        [
            "local_file: 4",
            "random_id: 4",
            "random_integer: 3",
            "random_pet: 3",
            "random_string: 2",
        ]
    );
    assert!(modules.iter().all(|row| !row.starts_with("other:")));
    assert!(types.iter().all(|row| !row.starts_with("other:")));
    assert_eq!(modules.iter().copied().map(row_count).sum::<usize>(), 16);
    assert_eq!(types.iter().copied().map(row_count).sum::<usize>(), 16);
    assert!(!stdout.contains("null_resource"));
    assert!(!stdout.contains("terraform_data"));
}

#[test]
fn fixture_top_3_merges_the_rest_into_other() {
    let stdout = assert_count_ok(&run(&["count", "--top", "3", FIXTURE]));
    let modules = group(&stdout, "modules");
    let types = group(&stdout, "types");
    assert_eq!(
        &modules[..3],
        ["module.compute: 5", "module.network: 5", "module.app: 3",]
    );
    assert_eq!(other_count(&modules), 2 + 1);
    assert_eq!(
        &types[..3],
        ["local_file: 4", "random_id: 4", "random_integer: 3",]
    );
    assert_eq!(other_count(&types), 3 + 2);
    assert!(!types.iter().any(|row| row.starts_with("random_pet:")));
    assert!(!modules.iter().any(|row| row.starts_with("module.data:")));
    assert!(!modules.iter().any(|row| row.starts_with("(root):")));
    assert_eq!(modules.iter().copied().map(row_count).sum::<usize>(), 16);
    assert_eq!(types.iter().copied().map(row_count).sum::<usize>(), 16);
}

#[test]
fn two_copies_of_the_fixture_are_a_duplicate_address() {
    let output = run(&["count", "--top", "3", FIXTURE, FIXTURE]);
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(output.stdout.is_empty(), "{stderr}");
    assert_eq!(
        stderr,
        "rum-meter: tests/fixtures/v4_top.json: duplicate address null_resource.skip[0]\n"
    );
}
