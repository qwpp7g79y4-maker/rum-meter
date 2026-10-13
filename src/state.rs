//! Terraform state v4 reader.
//!
//! Only `version`, resource identity, `index_key`, the `deposed` key and
//! whether `status` is exactly `tainted` are deserialized. Any other status
//! is dropped. Attribute values, sensitive attributes, private data and
//! outputs are not stored.
//!
//! The root value has to be a JSON object. A JSON array is refused: serde
//! would otherwise read the elements as positional fields.
//!
//! A leading UTF-8 BOM is skipped. UTF-16 and UTF-32, with or without a BOM,
//! are refused: the file has to be saved as UTF-8. The bytes are not decoded.
//! See [`crate::encoding`].
//!
//! Only a regular file is read. [`std::fs::metadata`] and `is_file` run before
//! [`std::fs::read`]. A directory, a FIFO or a device is refused without
//! reading a byte.

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Fields of a state file that a RUM count needs.
///
/// Unknown JSON fields are ignored on purpose, so values under `attributes`,
/// `sensitive_attributes`, `private` and `outputs` are never stored.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct State {
    pub version: u64,
    #[serde(default)]
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Resource {
    pub mode: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub name: String,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub instances: Vec<Instance>,
}

/// One resource instance.
///
/// `index_key` is absent when the resource has no count or for_each.
/// `deposed` is only the key of a create_before_destroy object.
/// `tainted` is true only when `status` is the string `tainted`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "InstanceFile")]
pub struct Instance {
    pub index_key: Option<IndexKey>,
    pub deposed: Option<String>,
    pub tainted: bool,
}

/// Raw instance fields. `status` exists only long enough to become a boolean.
#[derive(Deserialize)]
struct InstanceFile {
    #[serde(default)]
    index_key: Option<IndexKey>,
    #[serde(default)]
    deposed: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

impl From<InstanceFile> for Instance {
    fn from(raw: InstanceFile) -> Self {
        Self {
            index_key: raw.index_key,
            deposed: raw.deposed.filter(|key| !key.is_empty()),
            tainted: raw.status.as_deref() == Some("tainted"),
        }
    }
}

/// Count index (JSON number) or for_each key (JSON string).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum IndexKey {
    Number(u64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    Unreadable {
        path: PathBuf,
        kind: &'static str,
    },
    /// The path exists and is not a regular file. Nothing was read.
    NotRegular {
        path: PathBuf,
    },
    InvalidJson {
        path: PathBuf,
        line: usize,
        column: usize,
    },
    UnsupportedVersion {
        path: PathBuf,
    },
    /// Valid JSON whose root is an array, not an object.
    NotObject {
        path: PathBuf,
    },
    /// UTF-16 or UTF-32. Nothing was decoded.
    NotUtf8 {
        path: PathBuf,
        encoding: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParseError {
    InvalidJson { line: usize, column: usize },
    UnsupportedVersion,
    NotObject,
    NotUtf8(crate::encoding::Wide),
}

impl LoadError {
    fn invalid_json(path: &Path, line: usize, column: usize) -> Self {
        Self::InvalidJson {
            path: path.to_path_buf(),
            line,
            column,
        }
    }

    fn unsupported_version(path: &Path) -> Self {
        Self::UnsupportedVersion {
            path: path.to_path_buf(),
        }
    }

    fn not_utf8(path: &Path, wide: crate::encoding::Wide) -> Self {
        Self::NotUtf8 {
            path: path.to_path_buf(),
            encoding: wide.name(),
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, kind } => {
                write!(
                    f,
                    "rum-meter: {}: unreadable ({kind})",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::NotRegular { path } => {
                write!(
                    f,
                    "rum-meter: {}: not a regular file",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::InvalidJson { path, line, column } => {
                write!(
                    f,
                    "rum-meter: {}: invalid json at line {line} column {column}",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::UnsupportedVersion { path } => {
                write!(
                    f,
                    "rum-meter: {}: state version is not 4",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::NotObject { path } => {
                write!(
                    f,
                    "rum-meter: {}: state is not an object",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::NotUtf8 { path, encoding } => {
                write!(
                    f,
                    "rum-meter: {}: file is {encoding}; save it as UTF-8",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// Read one state file. The error text is only the path, the error kind, and
/// for JSON the line and column. It never includes bytes from the file.
///
/// A path that is not a regular file is refused before [`fs::read`].
pub fn load_state(path: &Path) -> Result<State, LoadError> {
    reject_unless_regular_file(path)?;
    let bytes = fs::read(path).map_err(|err| LoadError::Unreadable {
        path: path.to_path_buf(),
        kind: io_kind_label(err.kind()),
    })?;
    parse_bytes(&bytes).map_err(|err| match err {
        ParseError::InvalidJson { line, column } => LoadError::invalid_json(path, line, column),
        ParseError::UnsupportedVersion => LoadError::unsupported_version(path),
        ParseError::NotObject => LoadError::NotObject {
            path: path.to_path_buf(),
        },
        ParseError::NotUtf8(wide) => LoadError::not_utf8(path, wide),
    })
}

fn parse_bytes(bytes: &[u8]) -> Result<State, ParseError> {
    let bytes = crate::encoding::utf8_body(bytes).map_err(ParseError::NotUtf8)?;
    if json_root_is_array(bytes) {
        return match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(_) => Err(ParseError::NotObject),
            Err(err) => Err(ParseError::InvalidJson {
                line: err.line(),
                column: err.column(),
            }),
        };
    }
    let state: State = serde_json::from_slice(bytes).map_err(|err| ParseError::InvalidJson {
        line: err.line(),
        column: err.column(),
    })?;
    if state.version != 4 {
        return Err(ParseError::UnsupportedVersion);
    }
    Ok(state)
}

/// JSON whitespace, then `[`. An object starts with `{` and is left to serde.
fn json_root_is_array(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .copied()
        .find(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
        == Some(b'[')
}

/// `metadata().is_file()` before any read. A FIFO would block in `fs::read`
/// when nobody is writing, and `/dev/zero` would grow without a bound.
fn reject_unless_regular_file(path: &Path) -> Result<(), LoadError> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => Err(LoadError::NotRegular {
            path: path.to_path_buf(),
        }),
        Err(err) => Err(LoadError::Unreadable {
            path: path.to_path_buf(),
            kind: io_kind_label(err.kind()),
        }),
    }
}

fn io_kind_label(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NotFound => "not found",
        ErrorKind::PermissionDenied => "permission denied",
        ErrorKind::IsADirectory => "is a directory",
        _ => "io error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HANDWRITTEN: &[u8] = include_bytes!("../tests/fixtures/v4_handwritten.json");
    const CANARIES: &[&str] = &[
        "CANARY-SECRET-S-ATTR",
        "CANARY-SECRET-S-SENSITIVE",
        "CANARY-SECRET-S-PRIVATE",
        "CANARY-SECRET-S-OUTPUT",
    ];

    #[test]
    fn handwritten_state_parses_root_module_and_nested_instances() {
        let state = parse_bytes(HANDWRITTEN).expect("handwritten v4 state");
        assert_eq!(state.version, 4);
        assert_eq!(state.resources.len(), 4);

        let root = &state.resources[0];
        assert!(root.module.is_none());
        assert_eq!(root.mode, "managed");
        assert_eq!(root.resource_type, "null_resource");
        assert_eq!(root.name, "counted");
        assert_eq!(
            root.instances[0].index_key,
            Some(IndexKey::Number(0)),
            "count index is a JSON number"
        );
        assert_eq!(root.instances[1].index_key, Some(IndexKey::Number(3)));
        assert!(!root.instances[0].tainted);
        assert!(root.instances[0].deposed.is_none());

        let module = &state.resources[1];
        assert_eq!(module.module.as_deref(), Some("module.child"));
        assert_eq!(module.resource_type, "random_id");
        assert_eq!(module.name, "labeled");
        assert_eq!(
            module.instances[0].index_key,
            Some(IndexKey::String("web".to_string())),
            "for_each key is a JSON string"
        );
        assert_eq!(
            module.instances[1].index_key,
            Some(IndexKey::String("db".to_string()))
        );

        let nested = &state.resources[2];
        assert_eq!(nested.module.as_deref(), Some("module.child.module.nested"));
        assert_eq!(nested.mode, "data");
        assert_eq!(nested.resource_type, "local_file");
        assert_eq!(nested.name, "cfg");
        assert_eq!(nested.instances.len(), 1);
        assert_eq!(nested.instances[0].index_key, None);

        let empty = &state.resources[3];
        assert_eq!(empty.module.as_deref(), Some("module.child.module.nested"));
        assert_eq!(empty.resource_type, "terraform_data");
        assert!(empty.instances.is_empty());
    }

    #[test]
    fn parsed_state_does_not_retain_secret_fields() {
        let state = parse_bytes(HANDWRITTEN).expect("handwritten v4 state");
        let dumped = format!("{state:?}");
        for canary in CANARIES {
            assert!(
                !dumped.contains(canary),
                "parsed state retained a value from attributes, sensitive_attributes, private or outputs"
            );
        }
    }

    #[test]
    fn version_3_is_unsupported() {
        let err = parse_bytes(include_bytes!("../tests/fixtures/version_3.json")).unwrap_err();
        assert_eq!(err, ParseError::UnsupportedVersion);
        assert!(!format!("{err:?}").contains("CANARY-SECRET-S-V3"));
    }

    #[test]
    fn broken_json_is_invalid() {
        let err = parse_bytes(include_bytes!("../tests/fixtures/broken.json")).unwrap_err();
        assert!(matches!(err, ParseError::InvalidJson { .. }));
        assert!(!format!("{err:?}").contains("CANARY-SECRET-S-BROKEN"));
    }

    #[test]
    fn version_string_does_not_keep_the_value() {
        let err = parse_bytes(include_bytes!("../tests/fixtures/version_string.json")).unwrap_err();
        assert!(matches!(err, ParseError::InvalidJson { .. }));
        assert!(!format!("{err:?}").contains("CANARY-SECRET-S-TYPE"));
    }

    #[test]
    fn object_status_keeps_the_deposed_key_and_the_tainted_flag() {
        let state = parse_bytes(include_bytes!("../tests/fixtures/v4_object_status.json"))
            .expect("object status fixture");
        let current = &state.resources[0].instances[0];
        assert!(current.deposed.is_none());
        assert!(!current.tainted);
        assert_eq!(current.index_key, None);

        let deposed = &state.resources[0].instances[1];
        assert_eq!(deposed.deposed.as_deref(), Some("00000001"));
        assert!(!deposed.tainted);

        let tainted = &state.resources[1].instances[0];
        assert!(tainted.tainted);
        assert!(tainted.deposed.is_none());

        let dumped = format!("{state:?}");
        assert!(
            !dumped.contains("CANARY-SECRET"),
            "parsed state retained a value from attributes, sensitive_attributes, private, outputs or status"
        );
    }

    #[test]
    fn other_status_and_empty_deposed_are_dropped() {
        let state = parse_bytes(
            br#"{
                "version": 4,
                "resources": [{
                    "mode": "managed",
                    "type": "local_file",
                    "name": "a",
                    "instances": [
                        {"status": "CANARY-SECRET-OS-STATUS"},
                        {"status": ""},
                        {"deposed": ""},
                        {"status": "tainted", "deposed": "00000002"}
                    ]
                }]
            }"#,
        )
        .expect("inline state");
        let instances = &state.resources[0].instances;
        assert!(!instances[0].tainted);
        assert!(instances[0].deposed.is_none());
        assert!(!instances[1].tainted);
        assert!(instances[2].deposed.is_none());
        assert!(instances[3].tainted);
        assert_eq!(instances[3].deposed.as_deref(), Some("00000002"));
        assert!(
            !format!("{state:?}").contains("CANARY-SECRET"),
            "a status other than tainted was kept"
        );
    }

    #[test]
    fn status_that_is_not_a_string_is_invalid_json_without_its_value() {
        let err = parse_bytes(
            br#"{"version":4,"resources":[{"mode":"managed","type":"local_file","name":"a","instances":[{"status":{"token":"CANARY-SECRET-OS-STATUS"}}]}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, ParseError::InvalidJson { .. }));
        assert!(
            !format!("{err:?}").contains("CANARY-SECRET"),
            "parse error retained a status value"
        );
    }

    #[test]
    fn empty_file_is_invalid_json() {
        let err = parse_bytes(include_bytes!("../tests/fixtures/empty.json")).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidJson { line: 1, column: 0 }
        ));
    }

    #[test]
    fn json_array_state_is_refused() {
        let err = parse_bytes(br#"[4,[["managed","random_id","x",null,[[null,null,null]]]]]"#)
            .unwrap_err();
        assert_eq!(err, ParseError::NotObject);
    }
}
