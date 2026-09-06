//! Repository read settings, checked before a query opens the projection.

use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::FreshnessPolicy;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use serde_json::{Map, Value};

#[derive(Debug, Default)]
pub struct Settings {
    pub read: ReadSettings,
}

#[derive(Debug, Default)]
pub struct ReadSettings {
    pub freshness_policy: Option<FreshnessPolicy>,
    pub scan_limit: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("{path}: cannot read settings: {error}")]
    Unreadable {
        path: Utf8PathBuf,
        #[source]
        error: std::io::Error,
    },
    #[error("{path}: {key} {problem}")]
    Invalid {
        path: Utf8PathBuf,
        key: String,
        problem: String,
    },
}

impl Settings {
    /// Invalid settings refuse before a query reads the projection.
    #[rule("rule_invalid_read_setting_is_a_typed_refusal")]
    pub fn load(layout: &ProvenanceLayout) -> Result<Self, SettingsError> {
        let path = layout.provenance_dir().join("settings.json");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(SettingsError::Unreadable { path, error }),
        };
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(&path, "$", format!("must be valid JSON: {error}")))?;
        let root = object(&path, "$", &value)?;
        check_keys(&path, "", root, &["read"])?;
        let Some(read) = root.get("read") else {
            return Ok(Self::default());
        };
        let read = object(&path, "read", read)?;
        check_keys(&path, "read.", read, &["freshness_policy", "scan_limit"])?;
        let freshness_policy = read
            .get("freshness_policy")
            .map(|value| {
                value
                    .as_str()
                    .and_then(|word| serde_json::from_value(Value::String(word.to_owned())).ok())
                    .ok_or_else(|| {
                        invalid(
                            &path,
                            "read.freshness_policy",
                            format!(
                            "must be one of catch_up, annotate_only, refuse_stale (found {value})"
                        ),
                        )
                    })
            })
            .transpose()?;
        let scan_limit = read
            .get("scan_limit")
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|limit| usize::try_from(limit).ok())
                    .filter(|limit| *limit >= 1)
                    .ok_or_else(|| invalid(&path, "read.scan_limit", format!(
                        "must be a whole number of at least 1 that fits this platform (found {value})"
                    )))
            })
            .transpose()?;
        Ok(Self {
            read: ReadSettings {
                freshness_policy,
                scan_limit,
            },
        })
    }
}

fn invalid(path: &Utf8Path, key: &str, problem: String) -> SettingsError {
    SettingsError::Invalid {
        path: path.to_owned(),
        key: key.to_owned(),
        problem,
    }
}

fn object<'a>(
    path: &Utf8Path,
    key: &str,
    value: &'a Value,
) -> Result<&'a Map<String, Value>, SettingsError> {
    value
        .as_object()
        .ok_or_else(|| invalid(path, key, format!("must be an object (found {value})")))
}

fn check_keys(
    path: &Utf8Path,
    prefix: &str,
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), SettingsError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(invalid(
                path,
                &format!("{prefix}{key}"),
                format!("is unknown; allowed keys: {}", allowed.join(", ")),
            ));
        }
    }
    Ok(())
}
