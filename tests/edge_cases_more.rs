//! Edge cases the format allows that `tests/edge_cases.rs` does not cover yet:
//! newline-only files, a lone CR, an escaped NUL byte, Unicode spaces and
//! lookalike letters, duplicate JSON keys outside price tiers (rejected as
//! invalid JSON, same as `cost::tier_has_duplicate_key` rejects them in a
//! tier), deeply nested JSON, and the numeric oddities `1e309`, `-0` and an
//! out-of-range integer.
//!
//! Every state and price file here is synthetic, written by the test, and
//! deleted along with its temp directory by `cargo test`'s target dir cleanup.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const CANARY: &str = "CANARY-SECRET-ECM";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .output()
        .expect("rum-meter binary")
}

fn write_temp(group: &str, file: &str, body: &str) -> String {
    write_bytes(group, file, body.as_bytes())
}

/// Windows refuses a path part named like a device (`NUL`, `CON`, `COM1`, ...),
/// with or without an extension and in any letter case, so a test that writes
/// there passes on Linux and macOS but fails on Windows CI.
fn is_windows_reserved(part: &str) -> bool {
    let stem = part.split('.').next().unwrap_or(part).to_ascii_uppercase();
    let stem = stem.trim_end_matches([' ', '.']);
    matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
}

fn write_bytes(group: &str, file: &str, body: &[u8]) -> String {
    assert!(
        !is_windows_reserved(group) && !is_windows_reserved(file),
        "temp path part {group:?}/{file:?} is a reserved device name on Windows"
    );
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("edge-cases-more")
        .join(group);
    fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(file);
    fs::write(&path, body).expect("write temp file");
    path.into_os_string().into_string().expect("utf-8 path")
}

fn assert_input_error(label: &str, output: &Output) {
    let stderr = String::from_utf8(output.stderr.clone()).unwrap_or_else(|_| {
        panic!("{label}: stderr is not utf-8");
    });
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(2),
        "{label}: stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(output.stdout.is_empty(), "{label}: stdout:\n{stdout}");
    assert!(!stderr.contains(CANARY), "{label}: {stderr}");
    assert!(!stderr.contains("panicked"), "{label}: {stderr}");
}

fn assert_ok(label: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{label}: stderr:\n{stderr}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stderr.is_empty(), "{label}: stderr:\n{stderr}");
    String::from_utf8(output.stdout.clone())
        .unwrap_or_else(|_| panic!("{label}: stdout is not utf-8"))
}

#[test]
fn newline_only_files_are_invalid_json_not_a_panic() {
    for (label, body) in [
        ("lf", "\n\n\n"),
        ("crlf", "\r\n\r\n"),
        ("mixed", "\n\r\n\n\r"),
    ] {
        let path = write_temp("newlines-only", &format!("{label}.json"), body);
        assert_input_error(label, &run(&["count", &path]));
    }
}

#[test]
fn lone_cr_line_endings_count_the_same_as_lf() {
    let lf_body = "{\n  \"version\": 4,\n  \"resources\": [\n    {\n      \"mode\": \"managed\",\n      \"type\": \"random_id\",\n      \"name\": \"one\",\n      \"instances\": [{\"index_key\": 0}, {\"index_key\": 1}]\n    }\n  ]\n}\n";
    let lone_cr = lf_body.replace('\n', "\r");
    let lf = write_temp("lone-cr", "lf.json", lf_body);
    let cr = write_temp("lone-cr", "cr.json", &lone_cr);
    let lf_stdout = assert_ok("lf", &run(&["count", &lf]));
    let cr_stdout = assert_ok("cr", &run(&["count", &cr]));
    assert!(lf_stdout.contains("total rum: 2\n"), "{lf_stdout}");
    assert!(cr_stdout.contains("total rum: 2\n"), "{cr_stdout}");
}

#[test]
fn embedded_nul_byte_in_a_for_each_key_is_escaped_in_text_and_kept_in_json() {
    // A control character inside a JSON string must be the `\u00XX` escape:
    // JSON forbids a literal NUL byte in a string, same as any other control byte.
    let body = r#"{"version":4,"resources":[{"mode":"managed","type":"local_file","name":"a","instances":[{"index_key":"x\u0000y","attributes":{"token":"CANARY-SECRET-ECM-NUL"}}]}]}"#;
    let path = write_temp("nul-byte", "state.json", body);
    let text = assert_ok("nul-text", &run(&["count", "--explain", &path]));
    assert!(
        !text.as_bytes().contains(&0u8),
        "text report contains a raw NUL byte"
    );
    assert!(
        text.contains("local_file.a[\"x\\u0000y\"] counts R1\n"),
        "{text}"
    );
    assert!(!text.contains(CANARY));

    let json = run(&["count", "--json", "--explain", &path]);
    assert_eq!(json.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).expect("json");
    let address = value["instances"][0]["address"].as_str().expect("address");
    assert_eq!(address, "local_file.a[\"x\u{0}y\"]");
}

#[test]
fn line_separator_in_a_resource_name_is_escaped_in_text_and_kept_in_json() {
    // Reported: a resource name carrying U+2028 (LINE SEPARATOR) printed raw
    // in the text report. U+2028 is category Zl, not Cc or Cf, so
    // `escape_controls` did not touch it even though several renderers treat
    // it as a line break, letting a crafted name fake an extra report line.
    let body = r#"{"version":4,"resources":[{"mode":"managed","type":"local_file","name":"fake rum: 999999","instances":[{}]}]}"#;
    let path = write_temp("line-sep", "state.json", body);
    let text = assert_ok("line-sep-text", &run(&["count", "--explain", &path]));
    assert!(
        !text.contains('\u{2028}'),
        "text report contains a raw U+2028 line separator: {text:?}"
    );
    assert!(
        text.contains("local_file.fake\\u2028rum: 999999 counts R1\n"),
        "{text}"
    );

    let json = run(&["count", "--json", "--explain", &path]);
    assert_eq!(json.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).expect("json");
    let address = value["instances"][0]["address"].as_str().expect("address");
    assert_eq!(address, "local_file.fake\u{2028}rum: 999999");
}

#[test]
fn nul_byte_outside_a_string_is_invalid_json_not_a_panic() {
    let body = b"{\"version\":4,\"resources\":[]\x00}";
    let path = write_bytes("nul-byte", "structural.json", body);
    assert_input_error("nul-structural", &run(&["count", &path]));
}

#[test]
fn unicode_spaces_and_lookalike_letters_pass_through_unescaped() {
    // U+00A0 NBSP, U+2003 EM SPACE, U+3000 IDEOGRAPHIC SPACE: all Zs (space
    // separator), not Cc/Cf, so `escape_controls` must leave them alone.
    // Cyrillic "а" (U+0430) and Greek "ο" (U+03BF) look like Latin a/o.
    let key = "pl\u{00A0}an\u{2003}\u{0430}dmin\u{3000}\u{03BF}k";
    let body = format!(
        "{{\"version\":4,\"resources\":[{{\"mode\":\"managed\",\"type\":\"local_file\",\"name\":\"a\",\"instances\":[{{\"index_key\":\"{key}\",\"attributes\":{{\"token\":\"{CANARY}\"}}}}]}}]}}"
    );
    let path = write_temp("unicode-space", "state.json", &body);
    let text = assert_ok("unicode-space-text", &run(&["count", "--explain", &path]));
    assert!(
        text.contains(&format!("local_file.a[\"{key}\"] counts R1\n")),
        "{text}"
    );
    assert!(!text.contains(CANARY));

    let json = run(&["count", "--json", "--explain", &path]);
    assert_eq!(json.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).expect("json");
    let address = value["instances"][0]["address"].as_str().expect("address");
    assert_eq!(address, format!("local_file.a[\"{key}\"]"));
}

#[test]
fn duplicate_instance_status_key_is_invalid_json_without_leaking_either_value() {
    // The derived `Deserialize` for a struct field rejects a repeated key as
    // a duplicate-field error, not "last value wins": `count` must still
    // report it as plain invalid JSON, and leak neither of the two values.
    let body = format!(
        r#"{{"version":4,"resources":[{{"mode":"managed","type":"local_file","name":"a","instances":[{{"status":"{CANARY}-FIRST","status":"tainted"}}]}}]}}"#
    );
    let path = write_temp("dup-key", "instance.json", &body);
    assert_input_error("dup-instance", &run(&["count", &path]));
}

#[test]
fn duplicate_resource_type_key_is_invalid_json_without_leaking_either_value() {
    let body = r#"{"version":4,"resources":[{"mode":"managed","type":"CANARY-SECRET-ECM-FIRSTTYPE","type":"local_file","name":"a","instances":[{}]}]}"#;
    let path = write_temp("dup-key", "resource.json", body);
    assert_input_error("dup-resource", &run(&["count", &path]));
}

#[test]
fn deeply_nested_attribute_json_does_not_crash_and_does_not_leak() {
    let depth = 20_000;
    let mut nested = String::with_capacity(depth * 2 + CANARY.len() + 16);
    nested.push_str(&"[".repeat(depth));
    nested.push('"');
    nested.push_str(CANARY);
    nested.push_str("-DEEP\"");
    nested.push_str(&"]".repeat(depth));
    let body = format!(
        r#"{{"version":4,"resources":[{{"mode":"managed","type":"random_id","name":"x","instances":[{{"attributes":{nested}}}]}}]}}"#
    );
    let path = write_temp("nested", "state.json", &body);
    let output = run(&["count", "--explain", &path]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("utf-8");
    assert!(text.contains("total rum: 1\n"), "{text}");
    assert!(!text.contains(CANARY));
}

fn one_managed_state() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v4_one_managed.json")
        .into_os_string()
        .into_string()
        .expect("fixture path is utf-8")
}

#[test]
fn price_1e309_overflows_cleanly_without_inf_or_nan_in_the_message() {
    let prices = write_temp(
        "numbers",
        "huge-exponent.json",
        r#"{"currency":"EUR","tiers":[{"name":"huge","price_per_rum_hour":1e309}]}"#,
    );
    let state = one_managed_state();
    let output = run(&["count", "--prices", &prices, &state]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {prices}: monthly cost overflows\n")
    );
    assert!(!stderr.to_lowercase().contains("inf"));
    assert!(!stderr.to_lowercase().contains("nan"));
}

#[test]
fn negative_zero_price_is_accepted_as_zero_not_rejected_as_negative() {
    let prices = write_temp(
        "numbers",
        "neg-zero.json",
        r#"{"currency":"EUR","tiers":[{"name":"free","price_per_rum_hour":-0}]}"#,
    );
    let state = one_managed_state();
    let text = assert_ok("neg-zero", &run(&["count", "--prices", &prices, &state]));
    assert!(text.contains("free: EUR 0.00\n"), "{text}");
}

#[test]
fn index_key_number_past_u64_is_invalid_json_not_a_panic() {
    let body = r#"{"version":4,"resources":[{"mode":"managed","type":"random_id","name":"a","instances":[{"index_key":99999999999999999999999}]}]}"#;
    let path = write_temp("numbers", "huge-index.json", body);
    assert_input_error("huge-index", &run(&["count", &path]));
}

#[test]
fn windows_reserved_temp_names_are_refused_before_writing() {
    for reserved in [
        "nul",
        "NUL",
        "Nul",
        "nul.json",
        "NUL.txt",
        "con",
        "CON.json",
        "prn",
        "aux",
        "aux.state.json",
        "com1",
        "COM9",
        "lpt1",
        "LPT9.json",
    ] {
        assert!(
            is_windows_reserved(reserved),
            "{reserved} should be reserved"
        );
    }
    for allowed in [
        "nul-byte",
        "null",
        "nul_byte",
        "console",
        "auxiliary",
        "com0",
        "com10",
        "lpt",
        "state.json",
        "structural.json",
        "edge-cases-more",
    ] {
        assert!(!is_windows_reserved(allowed), "{allowed} should be allowed");
    }
}
