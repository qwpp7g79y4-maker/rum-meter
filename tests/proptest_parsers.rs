//! Property test: the state and price parsers never panic, no matter the bytes.
//!
//! Each case writes arbitrary bytes to a file and runs them through the real
//! public entry point (`load_state`/`load_prices`), so the whole pipeline
//! (file read, UTF-8/BOM check, JSON parsing, validation) is exercised the
//! same way a real file on disk would be.

use std::fs;
use std::path::PathBuf;

use proptest::prelude::*;

fn temp_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("proptest-io");
    fs::create_dir_all(&dir).expect("temp dir for proptest input files");
    dir.join(name)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn state_parser_never_panics_on_arbitrary_bytes(
        bytes in proptest::collection::vec(any::<u8>(), 0..2048),
    ) {
        let path = temp_path("state-proptest-input.json");
        fs::write(&path, &bytes).expect("write arbitrary bytes as a state file");
        let _ = rum_meter::state::load_state(&path);
    }

    #[test]
    fn price_parser_never_panics_on_arbitrary_bytes(
        bytes in proptest::collection::vec(any::<u8>(), 0..2048),
    ) {
        let path = temp_path("price-proptest-input.json");
        fs::write(&path, &bytes).expect("write arbitrary bytes as a price file");
        let _ = rum_meter::cost::load_prices(&path);
    }
}
