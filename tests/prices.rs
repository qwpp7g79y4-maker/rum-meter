use std::path::PathBuf;
use std::process::{Command, Output};

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

#[test]
fn invalid_price_file_exits_2() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("prices");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let prices = dir.join("empty-currency.json");
    std::fs::write(
        &prices,
        r#"{"currency":"","note":"CANARY-SECRET-PR-PRICES","tiers":[{"name":"small","price_per_rum_hour":1}]}"#,
    )
    .expect("write price file");
    let prices = prices.into_os_string().into_string().expect("utf-8");
    let state = fixture("v4_one_managed.json");
    let output = run(&["count", &state, "--prices", &prices]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid price file should exit 2"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty on an input error"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(stderr, format!("rum-meter: {prices}: currency is empty\n"));
    assert!(!stderr.contains("CANARY-SECRET-PR-PRICES"));
}

#[test]
fn duplicate_tier_name_exits_2_and_does_not_print_a_cost() {
    let prices = fixture("dup-tier-prices.json");
    let state = fixture("v4_one_managed.json");
    let output = run(&["count", &state, "--prices", &prices]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a price file with two tiers named the same should exit 2"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty on an input error"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {prices}: duplicate tier name\n")
    );
}

#[test]
fn oversized_currency_exits_2_without_printing_it() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("prices-wide-currency");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let prices = dir.join("wide-currency.json");
    let tiers = (0..800)
        .map(|i| format!(r#"{{"name":"t{i}","price_per_rum_hour":1}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        r#"{{"currency":"{}","note":"CANARY-SECRET-PR-WIDE","tiers":[{tiers}]}}"#,
        "C".repeat(40_000)
    );
    std::fs::write(&prices, &body).expect("write price file");
    let prices = prices.into_os_string().into_string().expect("utf-8");
    let state = fixture("v4_one_managed.json");
    let output = run(&["count", &state, "--prices", &prices]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a currency of 40000 characters should exit 2"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty, not one line per tier"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {prices}: currency is too long\n")
    );
    assert!(!stderr.contains("CANARY-SECRET-PR-WIDE"));
    assert!(!stderr.contains(&"C".repeat(16)));
}

#[test]
fn non_ascii_currency_exits_2() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("prices-euro");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let prices = dir.join("euro.json");
    std::fs::write(
        &prices,
        r#"{"currency":"€","note":"CANARY-SECRET-PR-EURO","tiers":[{"name":"small","price_per_rum_hour":1}]}"#,
    )
    .expect("write price file");
    let prices = prices.into_os_string().into_string().expect("utf-8");
    let state = fixture("v4_one_managed.json");
    let output = run(&["count", &state, "--prices", &prices]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {prices}: currency is not visible ASCII\n")
    );
    assert!(!stderr.contains('€'));
    assert!(!stderr.contains("CANARY-SECRET-PR-EURO"));
}
