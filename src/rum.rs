//! RUM rules R1-R4.
//!
//! Each instance gets an address, type, module, whether it counts, a rule code
//! and a reason. A deposed object keeps that rule and a distinct address
//! ending in ` (deposed KEY)`. A tainted object keeps R1 when it counts, and
//! the verdict records the tainted flag. A resource block with no instances
//! yields one R4 verdict and adds nothing to the total.
//!
//! Addresses are the identity `diff` uses. A state that repeats one is rejected
//! by the caller; [`find_duplicate_address`] reports the first repeat.

use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;

use crate::state::{IndexKey, Instance, Resource, State};

/// Why an instance does or does not count as billable RUM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// `mode` is `managed`, and the type is not a null resource.
    R1,
    /// Not a managed resource. `mode` = `data` keeps the reason `data source`.
    /// Any other mode keeps R2 and uses the reason `unknown mode`.
    R2,
    /// Type is `null_resource` or `terraform_data`.
    R3,
    /// The resource block has no instances.
    R4,
}

impl Rule {
    pub const fn code(self) -> &'static str {
        match self {
            Self::R1 => "R1",
            Self::R2 => "R2",
            Self::R3 => "R3",
            Self::R4 => "R4",
        }
    }

    pub const fn reason(self) -> &'static str {
        match self {
            Self::R1 => "managed resource",
            Self::R2 => "data source",
            Self::R3 => "null resource",
            Self::R4 => "resource has no instances",
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// One RUM decision.
///
/// Rows with [`Rule::R4`] stand for a resource block that has no instances.
/// Every other row is one instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub address: String,
    /// Exact identity of the state object: module, mode, type, name, index
    /// key and deposed key, each kept apart. Two objects can print the same
    /// `address` (a resource named `a[0]` and resource `a` at index 0), so
    /// `diff` matches on this, never on the printed address.
    pub identity: String,
    pub resource_type: String,
    pub module: Option<String>,
    pub counts: bool,
    pub rule: Rule,
    pub reason: &'static str,
    /// True only when the state object has `status` = `tainted`.
    pub tainted: bool,
    /// Deposed key, when this object is not the current one.
    pub deposed: Option<String>,
}

/// Classify every resource in a state.
///
/// Precedence: a block with no instances is R4. Otherwise a null type is R3,
/// `mode` = `managed` is R1, `mode` = `data` is R2 with reason `data source`,
/// and any other mode is R2 with reason `unknown mode`. It still does not count.
pub fn classify(state: &State) -> Vec<Verdict> {
    let mut verdicts = Vec::new();
    for resource in &state.resources {
        if resource.instances.is_empty() {
            verdicts.push(verdict(resource, None, Rule::R4));
            continue;
        }
        let rule = rule_for_instances(resource);
        for instance in &resource.instances {
            verdicts.push(verdict(resource, Some(instance), rule));
        }
    }
    verdicts
}

/// Number of verdicts that count as billable RUM.
pub fn total_rum(verdicts: &[Verdict]) -> usize {
    verdicts.iter().filter(|verdict| verdict.counts).count()
}

/// Text form of an untrusted string. Control characters and invisible or
/// bidirectional format characters are written as `\uXXXX`, so input cannot
/// inject a terminal sequence or visually reorder a line. JSON keeps the
/// original characters and lets the JSON encoder escape them.
pub fn escape_controls(text: &str) -> Cow<'_, str> {
    if !text.chars().any(needs_escape) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if needs_escape(ch) {
            out.push_str(&format!("\\u{:04x}", u32::from(ch)));
        } else {
            out.push(ch);
        }
    }
    Cow::Owned(out)
}

/// Control characters (Unicode Cc), plus the format characters (Cf) that
/// are invisible or change text direction: the Arabic letter mark, zero-width
/// characters, bidirectional embeddings, overrides and isolates, invisible
/// operators and the byte order mark. Also U+2028 and U+2029 (categories Zl
/// and Zp), which are not control or format characters but are still a line
/// break in several renderers, so an address, path or label carrying one
/// could otherwise fake an extra line in a text report.
fn needs_escape(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{2029}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

/// First address that occurs more than once, in verdict order.
///
/// The check is the address string itself, so it covers a root object, a
/// module path, a count index, a for_each key and a deposed key the same way.
/// A data source address starts with `data.`, so it does not collide with the
/// managed resource of the same type and name. The same module, mode, type
/// and name still collides.
pub fn find_duplicate_address(verdicts: &[Verdict]) -> Option<&str> {
    let mut seen = HashSet::with_capacity(verdicts.len());
    for verdict in verdicts {
        if !seen.insert(verdict.address.as_str()) {
            return Some(verdict.address.as_str());
        }
    }
    None
}

fn rule_for_instances(resource: &Resource) -> Rule {
    if is_null_type(&resource.resource_type) {
        Rule::R3
    } else if resource.mode == "managed" {
        Rule::R1
    } else {
        Rule::R2
    }
}

/// `data` keeps [`Rule::reason`] (`data source`). Another non-managed mode
/// stays on R2 but is not described as a data source.
fn reason_for(resource: &Resource, rule: Rule) -> &'static str {
    if rule == Rule::R2 && resource.mode != "data" {
        "unknown mode"
    } else {
        rule.reason()
    }
}

fn is_null_type(resource_type: &str) -> bool {
    resource_type == "null_resource" || resource_type == "terraform_data"
}

fn verdict(resource: &Resource, instance: Option<&Instance>, rule: Rule) -> Verdict {
    let counts = rule == Rule::R1;
    let index = instance.and_then(|instance| instance.index_key.as_ref());
    let deposed = instance.and_then(|instance| instance.deposed.clone());
    let tainted = instance.is_some_and(|instance| instance.tainted);
    let identity = serde_json::json!([
        resource
            .module
            .as_deref()
            .filter(|module| !module.is_empty()),
        resource.mode,
        resource.resource_type,
        resource.name,
        match index {
            None => serde_json::Value::Null,
            Some(IndexKey::Number(n)) => serde_json::json!({ "n": n }),
            Some(IndexKey::String(key)) => serde_json::json!({ "s": key }),
        },
        deposed.as_deref().filter(|key| !key.is_empty()),
    ])
    .to_string();
    Verdict {
        identity,
        address: address(
            resource.module.as_deref(),
            &resource.mode,
            &resource.resource_type,
            &resource.name,
            index,
            deposed.as_deref(),
        ),
        resource_type: resource.resource_type.clone(),
        module: resource.module.clone(),
        counts,
        rule,
        reason: reason_for(resource, rule),
        tainted,
        deposed,
    }
}

/// `module.a.module.b.type.name`, plus `[n]` or `["key"]` when the instance has an index.
/// A data source is `data.type.name`, or `module.a.data.type.name` inside a module,
/// the same shape Terraform uses. A deposed key is appended as ` (deposed KEY)`.
fn address(
    module: Option<&str>,
    mode: &str,
    resource_type: &str,
    name: &str,
    index: Option<&IndexKey>,
    deposed: Option<&str>,
) -> String {
    let mut address = String::new();
    if let Some(module) = module.filter(|module| !module.is_empty()) {
        address.push_str(module);
        address.push('.');
    }
    if mode == "data" {
        address.push_str("data.");
    }
    address.push_str(resource_type);
    address.push('.');
    address.push_str(name);
    match index {
        None => {}
        Some(IndexKey::Number(n)) => {
            address.push('[');
            address.push_str(&n.to_string());
            address.push(']');
        }
        Some(IndexKey::String(key)) => push_quoted_key(&mut address, key),
    }
    if let Some(key) = deposed.filter(|key| !key.is_empty()) {
        address.push_str(" (deposed ");
        address.push_str(key);
        address.push(')');
    }
    address
}

/// for_each keys are quoted. `"` and `\` are escaped so the address stays one token.
fn push_quoted_key(address: &mut String, key: &str) {
    address.push_str("[\"");
    for ch in key.chars() {
        if ch == '\\' || ch == '"' {
            address.push('\\');
        }
        address.push(ch);
    }
    address.push_str("\"]");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::IndexKey;

    fn resource(
        mode: &str,
        resource_type: &str,
        name: &str,
        module: Option<&str>,
        index_keys: &[Option<IndexKey>],
    ) -> Resource {
        Resource {
            mode: mode.to_string(),
            resource_type: resource_type.to_string(),
            name: name.to_string(),
            module: module.map(str::to_string),
            instances: index_keys
                .iter()
                .cloned()
                .map(|index_key| Instance {
                    index_key,
                    deposed: None,
                    tainted: false,
                })
                .collect(),
        }
    }

    fn state(resources: Vec<Resource>) -> State {
        State {
            version: 4,
            resources,
        }
    }

    #[test]
    fn escape_controls_leaves_printable_unicode_alone_and_escapes_format_characters() {
        assert!(matches!(
            escape_controls("café/日本 data.local_file.a[\"ü\"]"),
            Cow::Borrowed(_)
        ));
        assert_eq!(
            escape_controls("a\u{1b}\u{85}\u{61c}\u{200b}\u{202e}\u{2060}\u{2069}\u{feff}z"),
            "a\\u001b\\u0085\\u061c\\u200b\\u202e\\u2060\\u2069\\ufeffz"
        );
    }

    #[test]
    fn escape_controls_escapes_line_and_paragraph_separators() {
        // Reported: U+2028 (LINE SEPARATOR) and U+2029 (PARAGRAPH SEPARATOR)
        // are not `is_control()` and were not in the Cf allow-list, so a
        // crafted address or path could print one raw. Several renderers
        // treat U+2028/U+2029 as a line break, which is exactly the "visually
        // reorder a line" injection this function exists to prevent.
        assert_eq!(
            escape_controls("fake\u{2028}rum: 999999"),
            "fake\\u2028rum: 999999"
        );
        assert_eq!(escape_controls("a\u{2029}z"), "a\\u2029z");
    }

    #[test]
    fn r1_managed_instance_counts() {
        let state = state(vec![resource(
            "managed",
            "random_id",
            "server",
            None,
            &[None],
        )]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 1);
        let verdict = &verdicts[0];
        assert!(verdict.counts);
        assert_eq!(verdict.rule, Rule::R1);
        assert_eq!(verdict.rule.code(), "R1");
        assert_eq!(verdict.reason, "managed resource");
        assert_eq!(verdict.resource_type, "random_id");
        assert_eq!(verdict.module, None);
        assert_eq!(verdict.address, "random_id.server");
        assert!(!verdict.tainted);
        assert!(verdict.deposed.is_none());
        assert_eq!(total_rum(&verdicts), 1);
    }

    #[test]
    fn r2_data_source_does_not_count() {
        let state = state(vec![resource(
            "data",
            "local_file",
            "cfg",
            Some("module.child"),
            &[None],
        )]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 1);
        let verdict = &verdicts[0];
        assert!(!verdict.counts);
        assert_eq!(verdict.rule, Rule::R2);
        assert_eq!(verdict.reason, "data source");
        assert_eq!(verdict.resource_type, "local_file");
        assert_eq!(verdict.module.as_deref(), Some("module.child"));
        assert_eq!(verdict.address, "module.child.data.local_file.cfg");
        assert_eq!(total_rum(&verdicts), 0);
    }

    #[test]
    fn unknown_mode_stays_r2_with_reason_unknown_mode() {
        let state = state(vec![
            resource("future", "local_file", "x", None, &[None]),
            resource("data", "local_file", "y", None, &[None]),
        ]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 2);
        assert!(!verdicts[0].counts);
        assert_eq!(verdicts[0].rule, Rule::R2);
        assert_eq!(verdicts[0].rule.code(), "R2");
        assert_eq!(verdicts[0].reason, "unknown mode");
        assert_eq!(verdicts[0].address, "local_file.x");
        assert!(!verdicts[1].counts);
        assert_eq!(verdicts[1].rule, Rule::R2);
        assert_eq!(verdicts[1].reason, "data source");
        assert_eq!(verdicts[1].address, "data.local_file.y");
        assert_eq!(total_rum(&verdicts), 0);
    }

    #[test]
    fn r3_null_resource_and_terraform_data_do_not_count() {
        let state = state(vec![
            resource(
                "managed",
                "null_resource",
                "wait",
                None,
                &[Some(IndexKey::Number(0))],
            ),
            resource(
                "managed",
                "terraform_data",
                "marker",
                Some("module.child"),
                &[Some(IndexKey::String("once".to_string()))],
            ),
        ]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 2);
        assert!(verdicts.iter().all(|verdict| {
            !verdict.counts && verdict.rule == Rule::R3 && verdict.reason == "null resource"
        }));
        assert_eq!(verdicts[0].resource_type, "null_resource");
        assert_eq!(verdicts[0].address, "null_resource.wait[0]");
        assert_eq!(verdicts[1].resource_type, "terraform_data");
        assert_eq!(
            verdicts[1].address,
            "module.child.terraform_data.marker[\"once\"]"
        );
        assert_eq!(total_rum(&verdicts), 0);
    }

    #[test]
    fn r4_resource_without_instances_counts_zero() {
        let state = state(vec![resource(
            "managed",
            "random_id",
            "absent",
            Some("module.child.module.nested"),
            &[],
        )]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 1);
        let verdict = &verdicts[0];
        assert!(!verdict.counts);
        assert_eq!(verdict.rule, Rule::R4);
        assert_eq!(verdict.reason, "resource has no instances");
        assert_eq!(verdict.resource_type, "random_id");
        assert_eq!(
            verdict.module.as_deref(),
            Some("module.child.module.nested")
        );
        assert_eq!(
            verdict.address,
            "module.child.module.nested.random_id.absent"
        );
        assert_eq!(total_rum(&verdicts), 0);
    }

    #[test]
    fn count_three_and_for_each_two_keys_count_five() {
        let state = state(vec![
            resource(
                "managed",
                "random_id",
                "counted",
                None,
                &[
                    Some(IndexKey::Number(0)),
                    Some(IndexKey::Number(1)),
                    Some(IndexKey::Number(2)),
                ],
            ),
            resource(
                "managed",
                "random_id",
                "labeled",
                Some("module.child"),
                &[
                    Some(IndexKey::String("web".to_string())),
                    Some(IndexKey::String("db".to_string())),
                ],
            ),
        ]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 5);
        assert!(verdicts.iter().all(|verdict| {
            verdict.counts && verdict.rule == Rule::R1 && verdict.reason == "managed resource"
        }));
        assert!(
            verdicts
                .iter()
                .take(3)
                .all(|verdict| verdict.module.is_none() && verdict.resource_type == "random_id")
        );
        assert!(verdicts.iter().skip(3).all(|verdict| {
            verdict.module.as_deref() == Some("module.child")
                && verdict.resource_type == "random_id"
        }));
        assert_eq!(
            verdicts
                .iter()
                .map(|verdict| verdict.address.as_str())
                .collect::<Vec<_>>(),
            vec![
                "random_id.counted[0]",
                "random_id.counted[1]",
                "random_id.counted[2]",
                "module.child.random_id.labeled[\"web\"]",
                "module.child.random_id.labeled[\"db\"]",
            ]
        );
        assert_eq!(total_rum(&verdicts), 5);
    }

    #[test]
    fn address_root_is_type_dot_name() {
        assert_eq!(
            address(None, "managed", "random_id", "server", None, None),
            "random_id.server"
        );
    }

    #[test]
    fn address_module_prefixes_the_module_path() {
        assert_eq!(
            address(
                Some("module.a"),
                "managed",
                "random_id",
                "server",
                None,
                None
            ),
            "module.a.random_id.server"
        );
    }

    #[test]
    fn address_nested_module_keeps_each_module_segment() {
        assert_eq!(
            address(
                Some("module.a.module.b"),
                "managed",
                "random_id",
                "server",
                None,
                None
            ),
            "module.a.module.b.random_id.server"
        );
    }

    #[test]
    fn address_count_uses_numeric_brackets() {
        assert_eq!(
            address(
                None,
                "managed",
                "random_id",
                "server",
                Some(&IndexKey::Number(0)),
                None
            ),
            "random_id.server[0]"
        );
        assert_eq!(
            address(
                Some("module.a.module.b"),
                "managed",
                "random_id",
                "server",
                Some(&IndexKey::Number(0)),
                None,
            ),
            "module.a.module.b.random_id.server[0]"
        );
    }

    #[test]
    fn address_for_each_quotes_the_key() {
        assert_eq!(
            address(
                None,
                "managed",
                "random_id",
                "server",
                Some(&IndexKey::String("key".to_string())),
                None,
            ),
            "random_id.server[\"key\"]"
        );
    }

    #[test]
    fn address_for_each_escapes_quote_and_backslash() {
        let key = IndexKey::String(r#"say"hi\there"#.to_string());
        assert_eq!(
            address(None, "managed", "random_id", "server", Some(&key), None),
            r#"random_id.server["say\"hi\\there"]"#
        );
    }

    fn marked(index_key: Option<IndexKey>, deposed: Option<&str>, tainted: bool) -> Instance {
        Instance {
            index_key,
            deposed: deposed.map(str::to_string),
            tainted,
        }
    }

    #[test]
    fn deposed_object_counts_under_r1_with_its_own_address() {
        let state = state(vec![Resource {
            mode: "managed".to_string(),
            resource_type: "local_file".to_string(),
            name: "a".to_string(),
            module: None,
            instances: vec![
                marked(None, None, false),
                marked(None, Some("00000001"), false),
                marked(Some(IndexKey::Number(0)), Some("00000002"), true),
                marked(
                    Some(IndexKey::String("k".to_string())),
                    Some("00000003"),
                    false,
                ),
            ],
        }]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 4);
        assert!(verdicts.iter().all(|verdict| {
            verdict.counts && verdict.rule == Rule::R1 && verdict.reason == "managed resource"
        }));
        assert!(!verdicts[0].tainted);
        assert!(verdicts[0].deposed.is_none());
        assert_eq!(verdicts[0].address, "local_file.a");
        assert!(!verdicts[1].tainted);
        assert_eq!(verdicts[1].deposed.as_deref(), Some("00000001"));
        assert_eq!(verdicts[1].address, "local_file.a (deposed 00000001)");
        assert!(verdicts[2].tainted);
        assert_eq!(verdicts[2].deposed.as_deref(), Some("00000002"));
        assert_eq!(verdicts[2].address, "local_file.a[0] (deposed 00000002)");
        assert!(!verdicts[3].tainted);
        assert_eq!(
            verdicts[3].address,
            "local_file.a[\"k\"] (deposed 00000003)"
        );
        assert_eq!(total_rum(&verdicts), 4);
    }

    #[test]
    fn tainted_object_keeps_r1_and_its_address() {
        let state = state(vec![Resource {
            mode: "managed".to_string(),
            resource_type: "local_file".to_string(),
            name: "b".to_string(),
            module: Some("module.child".to_string()),
            instances: vec![marked(None, None, true)],
        }]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 1);
        let verdict = &verdicts[0];
        assert!(verdict.counts);
        assert_eq!(verdict.rule, Rule::R1);
        assert!(verdict.tainted);
        assert!(verdict.deposed.is_none());
        assert_eq!(verdict.address, "module.child.local_file.b");
        assert_eq!(total_rum(&verdicts), 1);
    }

    #[test]
    fn deposed_null_resource_does_not_count() {
        let state = state(vec![Resource {
            mode: "managed".to_string(),
            resource_type: "null_resource".to_string(),
            name: "wait".to_string(),
            module: Some("module.a.module.b".to_string()),
            instances: vec![marked(None, Some("00000001"), true)],
        }]);
        let verdicts = classify(&state);
        assert_eq!(verdicts.len(), 1);
        let verdict = &verdicts[0];
        assert!(!verdict.counts);
        assert_eq!(verdict.rule, Rule::R3);
        assert!(verdict.tainted);
        assert_eq!(verdict.deposed.as_deref(), Some("00000001"));
        assert_eq!(
            verdict.address,
            "module.a.module.b.null_resource.wait (deposed 00000001)"
        );
        assert_eq!(total_rum(&verdicts), 0);
    }

    #[test]
    fn address_deposed_suffix_follows_the_index() {
        assert_eq!(
            address(None, "managed", "local_file", "a", None, Some("00000001")),
            "local_file.a (deposed 00000001)"
        );
        assert_eq!(
            address(
                Some("module.a.module.b"),
                "managed",
                "local_file",
                "a",
                Some(&IndexKey::Number(0)),
                Some("00000001"),
            ),
            "module.a.module.b.local_file.a[0] (deposed 00000001)"
        );
        let key = IndexKey::String(r#"say"hi"#.to_string());
        assert_eq!(
            address(
                None,
                "managed",
                "local_file",
                "a",
                Some(&key),
                Some("00000001")
            ),
            "local_file.a[\"say\\\"hi\"] (deposed 00000001)"
        );
    }
}
