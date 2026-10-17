#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use rum_meter::render::{CountedFile, render_count_json, render_count_text};
use rum_meter::rum::classify;
use rum_meter::state::parse_bytes_for_fuzzing as parse_state_bytes;

/// A marker the tool promises to drop: it must never reach `{:?}` or a report.
const CANARY: &str = "FUZZ-FORBIDDEN-SECRET";

fuzz_target!(|data: &[u8]| {
    // Arbitrary bytes must never panic the parser, valid state or not.
    let _ = parse_state_bytes(data);

    if data.is_empty() {
        return;
    }
    // A syntactically valid state that plants the canary in every field the
    // module docs say is dropped: attributes, sensitive_attributes, private
    // and outputs. `data[0]` rotates through the variants so libFuzzer's byte
    // mutations still explore all of them.
    let body = match data[0] % 4 {
        0 => serde_json::json!({
            "version": 4,
            "resources": [{
                "mode": "managed", "type": "random_id", "name": "x",
                "instances": [{"attributes": {"a": CANARY}}],
            }],
        }),
        1 => serde_json::json!({
            "version": 4,
            "resources": [{
                "mode": "managed", "type": "random_id", "name": "x",
                "instances": [{"sensitive_attributes": [CANARY]}],
            }],
        }),
        2 => serde_json::json!({
            "version": 4,
            "resources": [{
                "mode": "managed", "type": "random_id", "name": "x",
                "instances": [{"private": CANARY}],
            }],
        }),
        _ => serde_json::json!({
            "version": 4,
            "resources": [{
                "mode": "managed", "type": "random_id", "name": "x",
                "instances": [{"status": "tainted"}],
            }],
            "outputs": {"o": {"value": CANARY, "sensitive": true}},
        }),
    };
    let bytes = serde_json::to_vec(&body).expect("serializing a json! value never fails");
    let Ok(state) = parse_state_bytes(&bytes) else {
        return;
    };
    assert!(
        !format!("{state:?}").contains(CANARY),
        "parsed state retained a field the docs promise to drop"
    );

    let verdicts = classify(&state);
    let path = Path::new("fuzz-state.json");
    let files = [CountedFile {
        path,
        verdicts: &verdicts,
    }];

    let text = render_count_text(&files, true, None, &[]);
    assert!(!text.contains(CANARY), "text report leaked a dropped field");

    let json = render_count_json(&files, true, None, &[]);
    assert!(!json.contains(CANARY), "json report leaked a dropped field");
    let _: serde_json::Value = serde_json::from_str(&json).expect("count json report must parse");
});
