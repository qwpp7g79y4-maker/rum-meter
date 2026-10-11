//! Monthly cost lines on `count`. Price files are written in the test temp dir.

use std::path::PathBuf;
use std::process::{Command, Output};

const ESTIMATE: &str = "estimate: HCP Terraform bills the hourly peak; this is a single snapshot";

/// Published HCP Terraform per-RUM-hour rates. A denylist for `src/`, not a price the tool uses.
const HASHICORP_RUM_HOUR_PRICES: &[&str] = &[
    "0.0001359",
    "0.0001418",
    "0.0001132",
    "0.0001182",
    "0.0001057",
    "0.0001103",
    "0.0000906",
    "0.0000945",
    "0.0000679",
    "0.0000709",
    "0.0000249",
    "0.0000260",
    "0.0000159",
    "0.0000165",
    "0.0000113",
    "0.0000118",
    "0.0009511",
    "0.0009925",
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .output()
        .expect("rum-meter binary")
}

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .into_os_string()
        .into_string()
        .expect("fixture path is utf-8")
}

fn write_prices() -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("costs");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("tiers.json");
    std::fs::write(
        &path,
        r#"{"currency":"EUR","note":"CANARY-SECRET-CO","tiers":[{"name":"alpha","price_per_rum_hour":1.25},{"name":"beta","price_per_rum_hour":0.0625}]}"#,
    )
    .expect("write price file");
    path.into_os_string().into_string().expect("utf-8")
}

fn stdout_ok(output: &Output) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is utf-8");
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("CANARY-SECRET"),
        "stdout leaked a fixture value:\n{stdout}"
    );
    assert!(
        stdout.ends_with(&format!("{ESTIMATE}\n")),
        "stdout:\n{stdout}"
    );
    stdout
}

fn cost_lines(stdout: &str) -> Vec<&str> {
    let mut lines = stdout.lines();
    assert!(
        lines.any(|line| line == "costs:"),
        "missing costs:\n{stdout}"
    );
    lines.take_while(|line| line.starts_with("  ")).collect()
}

fn has_two_decimal_amount(text: &str) -> bool {
    text.as_bytes().windows(4).any(|window| {
        window[0].is_ascii_digit()
            && window[1] == b'.'
            && window[2].is_ascii_digit()
            && window[3].is_ascii_digit()
    })
}

#[test]
fn hours_per_month_scales_each_tier_on_the_total_rum() {
    let prices = write_prices();
    let first = fixture("v4_handwritten.json");
    let second = fixture("v4_one_managed.json");
    let states = ["count", &first, &second, "--prices", &prices];

    let default_hours = stdout_ok(&run(&states));
    assert!(default_hours.lines().any(|line| line == "total rum: 3"));
    assert_eq!(
        cost_lines(&default_hours),
        ["  alpha: EUR 2737.50", "  beta: EUR 136.88"]
    );

    let ten_hours = stdout_ok(&run(&[
        "count",
        "--hours-per-month",
        "10",
        &first,
        &second,
        "--prices",
        &prices,
    ]));
    assert_eq!(
        cost_lines(&ten_hours),
        ["  alpha: EUR 37.50", "  beta: EUR 1.88"]
    );
}

#[test]
fn count_without_prices_shows_no_currency() {
    let first = fixture("v4_handwritten.json");
    let second = fixture("v4_one_managed.json");
    let stdout = stdout_ok(&run(&["count", &first, &second]));
    assert!(!stdout.contains("costs:"), "stdout:\n{stdout}");
    assert!(!stdout.contains("EUR") && !stdout.contains("USD") && !stdout.contains('$'));
    assert!(!has_two_decimal_amount(&stdout), "stdout:\n{stdout}");
}

#[test]
fn source_has_no_hashicorp_price_values() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![src];
    let mut rust_files = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("read src") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                rust_files.push(path);
            }
        }
    }
    assert!(!rust_files.is_empty(), "src has no rust files");
    for path in rust_files {
        let text = std::fs::read_to_string(&path).expect("read source");
        for price in HASHICORP_RUM_HOUR_PRICES {
            assert!(
                !text.contains(price),
                "{} contains a HashiCorp RUM-hour price",
                path.display()
            );
        }
    }
}
