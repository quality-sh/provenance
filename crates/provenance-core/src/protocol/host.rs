//! Connection metadata is the sole compatibility advertisement.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CompatibilityTuple {
    pub wire: u32,
    pub state: u32,
    pub review_journal: u32,
    pub read_derivation: u32,
}

pub const COMPATIBILITY: CompatibilityTuple = CompatibilityTuple {
    wire: super::SDK_PROTOCOL_VERSION,
    state: crate::SUPPORTED_SCHEMA_VERSION.0,
    review_journal: crate::review::REVIEW_SCHEMA_VERSION.0,
    read_derivation: 3,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PackageIdentity {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HostMetadata {
    pub compatibility: CompatibilityTuple,
    pub package: PackageIdentity,
    pub contract_digest: String,
    pub repository: Option<String>,
    pub scope: Option<String>,
}

impl HostMetadata {
    pub fn current(repository: Option<String>, scope: Option<String>) -> Self {
        let digest = Sha256::digest(include_bytes!("../../../../docs/api-contract-v2.md"));
        Self {
            compatibility: COMPATIBILITY,
            package: PackageIdentity {
                name: "provenance".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            contract_digest: format!("sha256:{digest:x}"),
            repository,
            scope,
        }
    }
}
