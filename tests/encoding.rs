//! Input as the three systems write it: UTF-16 and UTF-32 with and without a
//! BOM (Windows PowerShell), a UTF-8 BOM, CRLF line ends, file names that are
//! not valid UTF-8 (Linux), and paths with spaces and non-ASCII characters.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CANARY: &str = "CANARY-SECRET-ENCODING";

const STATE: &str = r#"{"version":4,"resources":[{"mode":"managed","type":"random_id","name":"one","instances":[{"index_key":0,"attributes":{"token":"CANARY-SECRET-ENCODING"}},{"index_key":1}]}]}"#;

const PRICES: &str = r#"{"note":"CANARY-SECRET-ENCODING","currency":"EUR","tiers":[{"name":"example-a","price_per_rum_hour":1}]}"#;

fn run<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .output()
        .expect("rum-meter binary")
}

fn dir(group: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("encoding")
        .join(group);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn write(group: &str, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir(group).join(name);
    fs::write(&path, bytes).expect("write temp file");
    path
}

fn encode(text: &str, utf32: bool, le: bool, bom: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let chars = bom.then_some('\u{FEFF}').into_iter().chain(text.chars());
    for c in chars {
        if utf32 {
            let unit = u32::from(c);
            out.extend(if le {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        } else {
            let mut buf = [0u16; 2];
            for unit in c.encode_utf16(&mut buf) {
                out.extend(if le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
        }
    }
    out
}

fn variants(text: &str) -> Vec<(String, &'static str, Vec<u8>)> {
    let mut out = Vec::new();
    for (utf32, name) in [(false, "UTF-16"), (true, "UTF-32")] {
        for le in [true, false] {
            for bom in [true, false] {
                let body = text.replace('\n', "\r\n");
                let label = format!(
                    "{}{}-{}",
                    if utf32 { "utf32" } else { "utf16" },
                    if le { "le" } else { "be" },
                    if bom { "bom" } else { "nobom" }
                );
                out.push((label, name, encode(&body, utf32, le, bom)));
            }
        }
    }
    out
}

fn no_canary(label: &str, bytes: &[u8]) {
    let stripped: Vec<u8> = bytes.iter().copied().filter(|&b| b != 0).collect();
    assert!(
        !String::from_utf8_lossy(&stripped).contains(CANARY),
        "{label}: canary in output"
    );
}

fn refused(label: &str, output: &Output, path: &Path, encoding: &str) {
    assert_eq!(output.status.code(), Some(2), "{label}");
    assert!(output.stdout.is_empty(), "{label}: stdout not empty");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "rum-meter: {}: file is {encoding}; save it as UTF-8\n",
            path.display()
        ),
        "{label}"
    );
    no_canary(label, &output.stdout);
    no_canary(label, &output.stderr);
}

#[test]
fn wide_state_files_are_refused_without_their_contents() {
    let good = write("state", "good.json", STATE.as_bytes());
    for (label, encoding, bytes) in variants(&format!("{STATE}\n")) {
        let path = write("state", &format!("{label}.json"), &bytes);
        for args in [
            vec![OsString::from("count"), path.clone().into()],
            vec!["count".into(), "--json".into(), path.clone().into()],
            vec!["count".into(), "--explain".into(), path.clone().into()],
            vec!["diff".into(), good.clone().into(), path.clone().into()],
            vec!["diff".into(), path.clone().into(), good.clone().into()],
        ] {
            let output = run(&args);
            refused(&format!("{label} {args:?}"), &output, &path, encoding);
        }
    }
}

#[test]
fn wide_price_files_are_refused_without_their_contents() {
    let state = write("prices", "state.json", STATE.as_bytes());
    for (label, encoding, bytes) in variants(&format!("{PRICES}\n")) {
        let path = write("prices", &format!("{label}.json"), &bytes);
        let output = run([
            OsStr::new("count"),
            OsStr::new("--prices"),
            path.as_os_str(),
            state.as_os_str(),
        ]);
        refused(&label, &output, &path, encoding);
    }
}

#[test]
fn utf8_bom_and_crlf_give_the_same_report() {
    let lf_state = write("lf", "state-lf.json", STATE.as_bytes());
    let lf_prices = write("lf", "prices-lf.json", PRICES.as_bytes());
    let baseline = run([
        OsStr::new("count"),
        OsStr::new("--json"),
        OsStr::new("--prices"),
        lf_prices.as_os_str(),
        lf_state.as_os_str(),
    ]);
    assert_eq!(baseline.status.code(), Some(0));
    let pretty_state = serde_json::to_string_pretty(
        &serde_json::from_str::<serde_json::Value>(STATE).expect("state"),
    )
    .expect("json");
    let pretty_prices = serde_json::to_string_pretty(
        &serde_json::from_str::<serde_json::Value>(PRICES).expect("prices"),
    )
    .expect("json");
    for (label, state_body, prices_body) in [
        (
            "bom",
            format!("\u{FEFF}{STATE}"),
            format!("\u{FEFF}{PRICES}"),
        ),
        (
            "crlf",
            pretty_state.replace('\n', "\r\n"),
            pretty_prices.replace('\n', "\r\n"),
        ),
        (
            "bom-crlf",
            format!("\u{FEFF}{}\r\n", pretty_state.replace('\n', "\r\n")),
            format!("\u{FEFF}{}\r\n", pretty_prices.replace('\n', "\r\n")),
        ),
    ] {
        // Same file names in another directory, so the paths in the report differ only there.
        let state = write(label, "state-lf.json", state_body.as_bytes());
        let prices = write(label, "prices-lf.json", prices_body.as_bytes());
        let output = run([
            OsStr::new("count"),
            OsStr::new("--json"),
            OsStr::new("--prices"),
            prices.as_os_str(),
            state.as_os_str(),
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Compare the parsed reports: the JSON escapes backslashes in Windows
        // paths, so a plain text replace of the path would not match there.
        let mut expected: serde_json::Value =
            serde_json::from_slice(&baseline.stdout).expect("baseline json");
        expected["inputs"] = serde_json::json!([state.display().to_string()]);
        let actual: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("report json");
        assert_eq!(actual, expected, "{label}");
    }
}

#[test]
fn paths_with_spaces_and_non_ascii_characters() {
    let path = write("names", "my state é ü 日本.json", STATE.as_bytes());
    let output = run([OsStr::new("count"), path.as_os_str()]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).expect("utf-8");
    assert!(text.contains("my state é ü 日本.json"), "{text}");
    assert!(text.contains("total rum: 2\n"), "{text}");
    let output = run([OsStr::new("count"), OsStr::new("--json"), path.as_os_str()]);
    assert_eq!(output.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert!(value.to_string().contains("my state é ü 日本.json"));
}

#[test]
fn usage_names_the_binary_rum_meter() {
    // Also on Windows, where the file is `rum-meter.exe`.
    let output = run(["count"]);
    assert_eq!(output.status.code(), Some(3));
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("Usage: rum-meter count "), "{err}");
    assert!(!err.contains("rum-meter.exe"), "{err}");
}

// macOS file systems refuse names that are not valid UTF-8 (EILSEQ).
#[cfg(all(unix, not(target_vendor = "apple")))]
#[test]
fn non_utf8_file_name_is_read() {
    use std::os::unix::ffi::OsStrExt;
    let path = dir("names").join(OsStr::from_bytes(b"state-\xff.json"));
    fs::write(&path, STATE).expect("write temp file");
    let output = run([OsStr::new("count"), path.as_os_str()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("text output is UTF-8");
    assert!(text.contains("state-\u{FFFD}.json"), "{text}");
    let output = run([OsStr::new("count"), OsStr::new("--json"), path.as_os_str()]);
    assert_eq!(output.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert!(value.to_string().contains("state-\u{FFFD}.json"));
    let output = run([OsStr::new("diff"), path.as_os_str(), path.as_os_str()]);
    assert_eq!(output.status.code(), Some(0));
}
