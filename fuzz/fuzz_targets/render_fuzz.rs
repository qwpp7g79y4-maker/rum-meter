#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use rum_meter::diff::diff_verdicts;
use rum_meter::render::{
    CountedFile, DiffJsonInput, render_count_json, render_count_text, render_diff_json,
    render_diff_text,
};
use rum_meter::rum::{Rule, Verdict};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let mid = data.len() / 2;
    let old_verdicts = build_verdicts(&data[..mid]);
    let new_verdicts = build_verdicts(&data[mid..]);

    let path = Path::new("fuzz.json");
    let files = [CountedFile {
        path,
        verdicts: &new_verdicts,
    }];
    let text = render_count_text(&files, true, Some(2), &[]);
    assert_fully_escaped(&text, "count text");
    let json = render_count_json(&files, true, Some(2), &[]);
    let _: serde_json::Value =
        serde_json::from_str(&json).expect("count json report must always parse");

    let diff = diff_verdicts(&old_verdicts, &new_verdicts);
    let old_path = Path::new("old.json");
    let new_path = Path::new("new.json");
    let diff_text = render_diff_text(old_path, new_path, &diff, &[]);
    assert_fully_escaped(&diff_text, "diff text");
    let diff_json = render_diff_json(
        &DiffJsonInput {
            old: old_path,
            new: new_path,
            old_verdicts: &old_verdicts,
            new_verdicts: &new_verdicts,
            diff: &diff,
            costs: &[],
            cost_deltas: &[],
        },
        true,
        Some(2),
    );
    let _: serde_json::Value =
        serde_json::from_str(&diff_json).expect("diff json report must always parse");
});

/// Verdicts whose address/module/type come straight from fuzz bytes, including
/// raw control characters, lone surrog-adjacent bytes (lossily converted) and
/// Unicode lookalikes, to stress the text renderer's escaping.
fn build_verdicts(data: &[u8]) -> Vec<Verdict> {
    let mut out = Vec::new();
    for chunk in data.chunks(8) {
        if chunk.len() < 2 {
            break;
        }
        let rule = match chunk[0] % 4 {
            0 => Rule::R1,
            1 => Rule::R2,
            2 => Rule::R3,
            _ => Rule::R4,
        };
        let raw = String::from_utf8_lossy(&chunk[1..]).into_owned();
        let address = if raw.is_empty() { "x".to_string() } else { raw };
        out.push(Verdict {
            address: address.clone(),
            identity: address,
            resource_type: "random_id".to_string(),
            module: None,
            counts: rule == Rule::R1,
            rule,
            reason: rule.reason(),
            tainted: chunk[0] % 2 == 0,
            deposed: None,
        });
    }
    out
}

/// `escape_controls` is the function the renderer itself uses to neutralize
/// control characters and invisible/bidi format characters in a field value.
/// If a rendered line still has something left for it to escape, a field
/// value reached the report unescaped. The line's own `writeln!`-inserted
/// `\n` is stripped by `lines()` first, since that is not a field value.
fn assert_fully_escaped(text: &str, label: &str) {
    for line in text.lines() {
        let escaped = rum_meter::rum::escape_controls(line);
        assert_eq!(
            escaped, line,
            "{label} has an unescaped control/format character on a line"
        );
    }
}
