//! Git locators on canonical records. Creation and update stamps are metadata.
use serde::{Deserialize, Deserializer, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub const COMMIT_PATTERN: &str = "^([0-9a-f]{40}|[0-9a-f]{64})$";

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    #[serde(deserialize_with = "deserialize_commit")]
    #[cfg_attr(feature = "schema", schemars(extend("pattern" = COMMIT_PATTERN)))]
    pub commit: String,
    #[serde(deserialize_with = "deserialize_at")]
    #[cfg_attr(feature = "schema", schemars(extend("format" = "date-time")))]
    pub at: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchivedStamp {
    #[serde(deserialize_with = "deserialize_commit")]
    #[cfg_attr(feature = "schema", schemars(extend("pattern" = COMMIT_PATTERN)))]
    pub commit: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_at"
    )]
    #[cfg_attr(feature = "schema", schemars(extend("format" = "date-time")))]
    pub at: Option<String>,
}

pub fn validate_commit(commit: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        matches!(commit.len(), 40 | 64)
            && commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "commit must be a full 40- or 64-digit lowercase hexadecimal hash"
    );
    Ok(())
}

fn validate_at(at: &str) -> anyhow::Result<()> {
    OffsetDateTime::parse(at, &Rfc3339)?;
    Ok(())
}

fn deserialize_commit<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let commit = String::deserialize(deserializer)?;
    validate_commit(&commit).map_err(serde::de::Error::custom)?;
    Ok(commit)
}

fn deserialize_at<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let at = String::deserialize(deserializer)?;
    validate_at(&at).map_err(serde::de::Error::custom)?;
    Ok(at)
}

fn deserialize_optional_at<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let at = Option::<String>::deserialize(deserializer)?;
    if let Some(at) = &at {
        validate_at(at).map_err(serde::de::Error::custom)?;
    }
    Ok(at)
}

impl super::Rule {
    pub fn validate_archive(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (self.status == super::RuleStatus::Archived) == self.archived_in_commit.is_some(),
            "archived_in_commit is required exactly when Rule status is archived"
        );
        if let Some(stamp) = &self.archived_in_commit {
            validate_commit(&stamp.commit)?;
            if let Some(at) = &stamp.at {
                validate_at(at)?;
            }
        }
        Ok(())
    }

    pub fn validate_transition(&self, previous: &Self) -> anyhow::Result<()> {
        anyhow::ensure!(
            previous.status != super::RuleStatus::Archived
                || self.status == super::RuleStatus::Archived,
            "archived Rule status is terminal"
        );
        self.validate_archive()
    }
}
