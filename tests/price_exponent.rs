//! A tiny price file with a huge exponent is decided at once.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_rum-meter");
const STATE: &str = "tests/fixtures/v4_one_managed.json";

// A counter, not the price text, names the directory: on a case-insensitive
// file system `1E+9` and `1e-9` would share one and the parallel tests would
// overwrite each other's price file.
static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn price_file(body: &str) -> std::path::PathBuf {
    let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rum-meter-exp-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prices.json");
    std::fs::write(&path, body).unwrap();
    path
}

fn with_price(price: &str) -> (std::process::Output, Duration) {
    let body =
        format!(r#"{{"currency":"EUR","tiers":[{{"name":"a","price_per_rum_hour":{price}}}]}}"#);
    let path = price_file(&body);
    let start = Instant::now();
    let out = Command::new(BIN)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["count", STATE, "--prices"])
        .arg(&path)
        .output()
        .unwrap();
    (out, start.elapsed())
}

#[test]
fn huge_positive_exponent_overflows_at_once() {
    for price in ["1e999999999", "1E+999999999"] {
        let (out, took) = with_price(price);
        assert_eq!(out.status.code(), Some(2), "{price}");
        assert!(out.stdout.is_empty(), "{price}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("overflows"),
            "{price}"
        );
        assert!(took < Duration::from_secs(5), "{price} took {took:?}");
    }
}

#[test]
fn huge_negative_exponent_is_zero_at_once() {
    for price in ["1e-999999999", "5E-999999999"] {
        let (out, took) = with_price(price);
        assert_eq!(out.status.code(), Some(0), "{price}");
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("EUR 0.00"),
            "{price}"
        );
        assert!(took < Duration::from_secs(5), "{price} took {took:?}");
    }
}

#[test]
fn ordinary_exponents_keep_their_exact_amounts() {
    for (price, amount) in [
        ("1e-2", "7.30"),
        ("5e-3", "3.65"),
        ("4.9e-3", "3.58"),
        ("1.5e0", "1095.00"),
    ] {
        let (out, _) = with_price(price);
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains(&format!("EUR {amount}\n")), "{price}: {text}");
    }
}
