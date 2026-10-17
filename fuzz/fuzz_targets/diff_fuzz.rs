#![no_main]

use libfuzzer_sys::fuzz_target;
use rum_meter::diff::diff_verdicts;
use rum_meter::rum::{classify, total_rum};
use rum_meter::state::{IndexKey, Instance, Resource, State};

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let mid = data.len() / 2;
    let old_state = build_state(&data[..mid]);
    let new_state = build_state(&data[mid..]);
    let old_verdicts = classify(&old_state);
    let new_verdicts = classify(&new_state);

    let diff = diff_verdicts(&old_verdicts, &new_verdicts);

    assert_eq!(
        diff.old_rum,
        total_rum(&old_verdicts),
        "StateDiff.old_rum diverged from an independent total_rum"
    );
    assert_eq!(
        diff.new_rum,
        total_rum(&new_verdicts),
        "StateDiff.new_rum diverged from an independent total_rum"
    );
    let old_i = i64::try_from(diff.old_rum).unwrap_or(i64::MAX);
    let new_i = i64::try_from(diff.new_rum).unwrap_or(i64::MAX);
    assert_eq!(diff.rum_delta(), new_i.saturating_sub(old_i));

    assert!(
        diff.added.windows(2).all(|w| w[0] <= w[1]),
        "added addresses are not sorted"
    );
    assert!(
        diff.removed.windows(2).all(|w| w[0] <= w[1]),
        "removed addresses are not sorted"
    );
    assert!(
        diff.changed_rule
            .windows(2)
            .all(|w| w[0].address <= w[1].address),
        "changed_rule is not sorted by address"
    );
});

/// A small deterministic state generator so byte mutations exercise every
/// resource/instance shape: modes, null types, modules, count/for_each
/// indices, deposed keys and the tainted flag.
fn build_state(data: &[u8]) -> State {
    let mut resources = Vec::new();
    for chunk in data.chunks(6) {
        if chunk.len() < 6 {
            break;
        }
        let mode = match chunk[0] % 3 {
            0 => "managed",
            1 => "data",
            _ => "future",
        };
        let resource_type = match chunk[1] % 4 {
            0 => "random_id",
            1 => "null_resource",
            2 => "terraform_data",
            _ => "local_file",
        };
        let module = if chunk[2] % 2 == 0 {
            None
        } else {
            Some(format!("module.m{}", chunk[2]))
        };
        let instance_count = (chunk[3] % 4) as usize;
        let mut instances = Vec::with_capacity(instance_count);
        for i in 0..instance_count {
            let index_key = match chunk[4] % 3 {
                0 => None,
                1 => Some(IndexKey::Number(i as u64)),
                _ => Some(IndexKey::String(format!("k{i}"))),
            };
            let deposed = if chunk[5] % 2 == 0 {
                None
            } else {
                Some(format!("{i:08}"))
            };
            let tainted = chunk[5] % 3 == 0;
            instances.push(Instance {
                index_key,
                deposed,
                tainted,
            });
        }
        resources.push(Resource {
            mode: mode.to_string(),
            resource_type: resource_type.to_string(),
            name: format!("r{}", chunk[0]),
            module,
            instances,
        });
    }
    State {
        version: 4,
        resources,
    }
}
