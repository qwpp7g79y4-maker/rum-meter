//! Edge cases.
//!
//! A valid state has one object per address. The address is what `diff` compares,
//! so a duplicate (root, module, count, for_each or deposed, in one resource or
//! across two blocks) is an input error on `count` and on `diff`.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const CANARY: &str = "CANARY-SECRET-EC";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .output()
        .expect("rum-meter binary")
}

fn write_temp(group: &str, file: &str, body: &str) -> String {
    write_bytes(group, file, body.as_bytes())
}

fn write_bytes(group: &str, file: &str, body: &[u8]) -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("edge-cases")
        .join(group);
    fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(file);
    fs::write(&path, body).expect("write temp file");
    path.into_os_string().into_string().expect("utf-8 path")
}

fn instance(index_json: Option<&str>, deposed: Option<&str>) -> String {
    let mut fields = vec![format!(r#""attributes":{{"token":"{CANARY}"}}"#)];
    if let Some(index) = index_json {
        fields.push(format!(r#""index_key":{index}"#));
    }
    if let Some(key) = deposed {
        fields.push(format!(r#""deposed":"{key}""#));
    }
    format!("{{{}}}", fields.join(","))
}

fn resource(mode: &str, module: Option<&str>, name: &str, instances: &str) -> String {
    let module = match module {
        Some(module) => format!(r#""module":"{module}","#),
        None => String::new(),
    };
    format!(
        r#"{{"mode":"{mode}","type":"local_file","name":"{name}",{module}"instances":[{instances}]}}"#
    )
}

fn state(resources: &[String]) -> String {
    format!(
        r#"{{"version":4,"lineage":"synthetic-ec","resources":[{}]}}"#,
        resources.join(",")
    )
}

fn assert_duplicate(path: &str, address: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(output.stdout.is_empty(), "stdout:\n{stdout}");
    assert_eq!(
        stderr,
        format!("rum-meter: {path}: duplicate address {address}\n")
    );
}

#[test]
fn duplicate_address_is_an_input_error_for_every_shape_and_for_diff() {
    // (label, module, index JSON, deposed key, expected address)
    let shapes = [
        ("root", None, None, None, "local_file.a"),
        (
            "module",
            Some("module.child.module.nested"),
            None,
            None,
            "module.child.module.nested.local_file.a",
        ),
        ("count", None, Some("1"), None, "local_file.a[1]"),
        (
            "for-each",
            Some("module.child"),
            Some(r#""web""#),
            None,
            r#"module.child.local_file.a["web"]"#,
        ),
        (
            "deposed",
            None,
            None,
            Some("00000001"),
            "local_file.a (deposed 00000001)",
        ),
        (
            "deposed-count",
            Some("module.a"),
            Some("0"),
            Some("00000001"),
            "module.a.local_file.a[0] (deposed 00000001)",
        ),
    ];

    for (label, module, index, deposed, address) in shapes {
        let one = instance(index, deposed);
        let two_blocks = state(&[
            resource("managed", module, "a", &one),
            resource("managed", module, "a", &one),
        ]);
        let path = write_temp(label, "blocks.json", &two_blocks);
        assert_duplicate(&path, address, &run(&["count", &path]));

        let two_instances = state(&[resource("managed", module, "a", &format!("{one},{one}"))]);
        let path = write_temp(label, "instances.json", &two_instances);
        assert_duplicate(&path, address, &run(&["count", &path]));
    }

    let counted = instance(Some("1"), None);
    let single = write_temp(
        "diff",
        "one.json",
        &state(&[resource("managed", None, "a", &counted)]),
    );
    let doubled = write_temp(
        "diff",
        "two.json",
        &state(&[resource(
            "managed",
            None,
            "a",
            &format!("{counted},{counted}"),
        )]),
    );
    assert_duplicate(
        &doubled,
        "local_file.a[1]",
        &run(&["diff", &single, &doubled]),
    );
    assert_duplicate(
        &doubled,
        "local_file.a[1]",
        &run(&["diff", &doubled, &single]),
    );
}

fn assert_count_line(output: &Output, address_line: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is utf-8");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(output.stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains("rum: 1\n"), "{stdout}");
    assert!(stdout.contains("total rum: 1\n"), "{stdout}");
    assert!(stdout.contains(address_line), "{stdout}");
    assert!(!stdout.contains(CANARY));
}

#[test]
fn managed_and_data_with_the_same_name_are_not_a_duplicate() {
    let one = instance(None, None);
    let root = write_temp(
        "data-addr",
        "root.json",
        &state(&[
            resource("managed", None, "a", &one),
            resource("data", None, "a", &one),
        ]),
    );
    let explained = run(&["count", "--explain", &root]);
    assert_count_line(&explained, "  local_file.a counts R1\n");
    assert_count_line(&explained, "  data.local_file.a does not count R2\n");

    let module = write_temp(
        "data-addr",
        "module.json",
        &state(&[
            resource("managed", Some("module.x"), "a", &one),
            resource("data", Some("module.x"), "a", &one),
        ]),
    );
    let explained = run(&["count", "--explain", &module]);
    assert_count_line(&explained, "  module.x.local_file.a counts R1\n");
    assert_count_line(
        &explained,
        "  module.x.data.local_file.a does not count R2\n",
    );

    let only_managed = write_temp(
        "data-addr",
        "managed.json",
        &state(&[resource("managed", None, "a", &one)]),
    );
    let diff = run(&["diff", &only_managed, &root]);
    let stderr = String::from_utf8_lossy(&diff.stderr);
    let stdout = String::from_utf8(diff.stdout.clone()).expect("stdout is utf-8");
    assert_eq!(
        diff.status.code(),
        Some(0),
        "stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(diff.stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains("rum delta: 0\n"), "{stdout}");
    assert!(stdout.contains("added:\nremoved:\n"), "{stdout}");

    let doubled = write_temp(
        "data-addr",
        "two-data.json",
        &state(&[
            resource("data", None, "a", &one),
            resource("data", None, "a", &one),
        ]),
    );
    assert_duplicate(&doubled, "data.local_file.a", &run(&["count", &doubled]));
    assert_duplicate(
        &doubled,
        "data.local_file.a",
        &run(&["diff", &only_managed, &doubled]),
    );
}

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .into_os_string()
        .into_string()
        .expect("fixture path is utf-8")
}

fn assert_cost_overflow(path: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(output.stdout.is_empty(), "stdout:\n{stdout}");
    assert_eq!(
        stderr,
        format!("rum-meter: {path}: monthly cost overflows\n")
    );
    assert!(!stderr.contains("184467440737095516"));
    assert!(!stderr.contains("1e308"));
    assert!(!stderr.contains(CANARY));
}

#[test]
fn cost_overflow_is_an_input_error_on_count_and_diff() {
    let prices = write_temp(
        "overflow",
        "prices.json",
        r#"{"currency":"EUR","note":"CANARY-SECRET-EC-PRICE","tiers":[{"name":"huge","price_per_rum_hour":1e308}]}"#,
    );
    let state = fixture("v4_one_managed.json");
    assert_cost_overflow(&prices, &run(&["count", "--prices", &prices, &state]));
    assert_cost_overflow(
        &prices,
        &run(&["diff", "--prices", &prices, &state, &state]),
    );
}

fn utf16_with_bom(text: &str, le: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2 + text.len() * 2);
    bytes.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
    for unit in text.encode_utf16() {
        let coded = if le {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        };
        bytes.extend_from_slice(&coded);
    }
    bytes
}

#[test]
fn utf8_bom_is_skipped_and_utf16_is_rejected() {
    let body = r#"{"version":4,"resources":[{"mode":"managed","type":"random_id","name":"one","instances":[{"attributes":{"token":"CANARY-SECRET-EC-UTF"}}]}]}"#;
    let plain = write_temp("encoding", "plain.json", body);
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(body.as_bytes());
    let with_bom = write_bytes("encoding", "bom.json", &bom);

    let plain_out = run(&["count", &plain]);
    let bom_out = run(&["count", &with_bom]);
    assert_eq!(plain_out.status.code(), Some(0));
    assert_eq!(
        bom_out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&bom_out.stderr)
    );
    let plain_stdout = String::from_utf8(plain_out.stdout).expect("utf-8");
    let bom_stdout = String::from_utf8(bom_out.stdout).expect("utf-8");
    assert!(plain_stdout.contains("total rum: 1\n"), "{plain_stdout}");
    assert!(bom_stdout.contains("total rum: 1\n"), "{bom_stdout}");
    assert!(!bom_stdout.contains(CANARY));
    assert!(bom_out.stderr.is_empty());

    for (name, bytes) in [
        ("utf16-le.json", utf16_with_bom(body, true)),
        ("utf16-be.json", utf16_with_bom(body, false)),
    ] {
        let path = write_bytes("encoding", name, &bytes);
        let output = run(&["count", &path]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
        assert_eq!(
            output.status.code(),
            Some(2),
            "stdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(output.stdout.is_empty(), "stdout:\n{stdout}");
        assert_eq!(
            stderr,
            format!("rum-meter: {path}: file is UTF-16; save it as UTF-8\n")
        );
    }
}

#[test]
fn control_characters_are_escaped_in_text_and_kept_in_json() {
    let body = r#"{"version":4,"resources":[{"mode":"managed","type":"local_file","name":"a","instances":[{"index_key":"\u001b[31m","attributes":{"token":"CANARY-SECRET-EC-ESC"}}]}]}"#;
    let path = write_temp("controls", "state.json", body);
    let empty = write_temp("controls", "empty.json", r#"{"version":4,"resources":[]}"#);

    let text = run(&["count", "--explain", &path]);
    assert_eq!(
        text.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&text.stderr)
    );
    assert!(
        !text.stdout.contains(&0x1b),
        "text output contains a raw ESC"
    );
    let stdout = String::from_utf8(text.stdout).expect("text stdout is utf-8");
    assert!(
        stdout.contains("  local_file.a[\"\\u001b[31m\"] counts R1\n"),
        "{stdout}"
    );
    assert!(!stdout.contains(CANARY));
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.contains("local_file.a"))
            .count(),
        1,
        "the address must stay on one line"
    );

    let json = run(&["count", "--json", "--explain", &path]);
    assert_eq!(json.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).expect("json");
    let address = value["instances"][0]["address"].as_str().expect("address");
    assert_eq!(address, "local_file.a[\"\u{1b}[31m\"]");

    let diff = run(&["diff", &empty, &path]);
    assert_eq!(
        diff.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    assert!(!diff.stdout.contains(&0x1b), "diff text contains a raw ESC");
    let diff_stdout = String::from_utf8(diff.stdout).expect("diff stdout is utf-8");
    assert!(
        diff_stdout.contains("  local_file.a[\"\\u001b[31m\"]\n"),
        "{diff_stdout}"
    );
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
    assert!(!stderr.contains("CANARY-SECRET"), "{label}: {stderr}");
    assert!(!stderr.contains("panicked"), "{label}: {stderr}");
    assert!(!stderr.contains("stack backtrace"), "{label}: {stderr}");
}

#[test]
fn broken_input_exits_2_without_panic_or_state_content() {
    let cases = [
        ("empty", fixture("empty.json")),
        (
            "object",
            write_temp("pin", "object.json", r#"{"note":"CANARY-SECRET-EC-OBJ"}"#),
        ),
        (
            "array",
            write_temp("pin", "array.json", r#"["CANARY-SECRET-EC-ARR"]"#),
        ),
        (
            "truncated",
            write_temp(
                "pin",
                "truncated.json",
                r#"{"version":4,"note":"CANARY-SECRET-EC-TRUNC""#,
            ),
        ),
        (
            "utf8",
            write_bytes(
                "pin",
                "bad-utf8.json",
                b"{\"version\":4,\"note\":\"CANARY-SECRET-EC-UTF8\",\xff}",
            ),
        ),
    ];
    for (label, path) in cases {
        assert_input_error(label, &run(&["count", &path]));
    }
}

#[test]
fn crlf_counts_the_same_as_lf() {
    let lf_body = "{\n  \"version\": 4,\n  \"resources\": [\n    {\n      \"mode\": \"managed\",\n      \"type\": \"random_id\",\n      \"name\": \"one\",\n      \"instances\": [{\"index_key\": 0}, {\"index_key\": 1}]\n    }\n  ]\n}\n";
    let lf = write_temp("pin", "lf.json", lf_body);
    let crlf = write_temp("pin", "crlf.json", &lf_body.replace('\n', "\r\n"));
    let lf_out = run(&["count", &lf]);
    let crlf_out = run(&["count", &crlf]);
    assert_eq!(lf_out.status.code(), Some(0));
    assert_eq!(crlf_out.status.code(), Some(0));
    let lf_stdout = String::from_utf8(lf_out.stdout).expect("utf-8");
    let crlf_stdout = String::from_utf8(crlf_out.stdout).expect("utf-8");
    for line in ["rum: 2\n", "total rum: 2\n"] {
        assert!(lf_stdout.contains(line), "{lf_stdout}");
        assert!(crlf_stdout.contains(line), "{crlf_stdout}");
    }
}

#[test]
fn version_3_and_5_exit_2() {
    for (label, path) in [
        ("v3", fixture("version_3.json")),
        (
            "v5",
            write_temp(
                "pin",
                "version5.json",
                r#"{"version":5,"note":"CANARY-SECRET-EC-V5","resources":[]}"#,
            ),
        ),
    ] {
        let output = run(&["count", &path]);
        assert_input_error(label, &output);
        let stderr = String::from_utf8(output.stderr).expect("utf-8");
        assert_eq!(
            stderr,
            format!("rum-meter: {path}: state version is not 4\n")
        );
    }
}

#[test]
fn missing_or_empty_instances_count_zero_under_r4() {
    let path = write_temp(
        "pin",
        "r4.json",
        r#"{"version":4,"resources":[{"mode":"managed","type":"local_file","name":"bare"},{"mode":"managed","type":"local_file","name":"empty","instances":[]}]}"#,
    );
    let output = run(&["count", "--explain", &path]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    assert!(stdout.contains("rum: 0\n"), "{stdout}");
    assert!(stdout.contains("total rum: 0\n"), "{stdout}");
    assert!(
        stdout.contains("not counted: R2 0, R3 0, R4 2\n"),
        "{stdout}"
    );
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.contains("does not count R4"))
            .count(),
        2,
        "{stdout}"
    );
}

#[test]
fn unknown_mode_does_not_count_under_r2() {
    let path = write_temp(
        "pin",
        "mode.json",
        r#"{"version":4,"resources":[{"mode":"future","type":"local_file","name":"x","instances":[{}]},{"mode":"data","type":"local_file","name":"y","instances":[{}]}]}"#,
    );
    let output = run(&["count", "--explain", &path]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    assert!(stdout.contains("rum: 0\n"), "{stdout}");
    assert!(stdout.contains("total rum: 0\n"), "{stdout}");
    assert!(
        stdout.contains("not counted: R2 2, R3 0, R4 0\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("local_file.x does not count R2\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("data.local_file.y does not count R2\n"),
        "{stdout}"
    );
    let state = rum_meter::state::load_state(std::path::Path::new(&path)).expect("state");
    let verdicts = rum_meter::rum::classify(&state);
    assert!(!verdicts[0].counts);
    assert_eq!(verdicts[0].rule, rum_meter::rum::Rule::R2);
    assert_eq!(verdicts[0].reason, "unknown mode");
    assert!(!verdicts[1].counts);
    assert_eq!(verdicts[1].rule, rum_meter::rum::Rule::R2);
    assert_eq!(verdicts[1].reason, "data source");
}

#[test]
fn large_state_counts_two_hundred_thousand_instances() {
    let path = write_bytes("pin", "large.json", &[]);
    let file = std::fs::File::create(&path).expect("create large state");
    let mut file = std::io::BufWriter::new(file);
    use std::io::Write;
    write!(
        file,
        r#"{{"version":4,"resources":[{{"mode":"managed","type":"random_id","name":"n","instances":["#
    )
    .expect("write header");
    for index in 0..200_000 {
        if index > 0 {
            file.write_all(b",").expect("write comma");
        }
        write!(file, r#"{{"index_key":{index}}}"#).expect("write instance");
    }
    file.write_all(b"]}]}").expect("write footer");
    file.flush().expect("flush large state");
    drop(file);

    let output = run(&["count", &path]);
    let _ = fs::remove_file(&path);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    assert!(stdout.contains("total rum: 200000\n"), "{stdout}");
    assert!(output.stderr.is_empty());
}
