//! Repository-free host compatibility metadata.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HostMetadata {
    pub engine_version: String,
    pub protocol_version: u32,
}

impl HostMetadata {
    pub fn current() -> Self {
        Self {
            engine_version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_version: super::SDK_PROTOCOL_VERSION,
        }
    }
}
