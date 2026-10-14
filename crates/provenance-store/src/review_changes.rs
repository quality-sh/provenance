//! Canonical record changes read under the repository publication guard.
mod diff;
mod snapshot;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Canonical records and configuration from one guarded read.
#[derive(Debug)]
pub struct ReviewStateSnapshot {
    records: BTreeMap<(String, String, String), Value>,
    manifest: Value,
    settings: Option<Value>,
}

/// Record identities that a reader must read again.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChangedRecord {
    pub kind: String,
    pub id: String,
}

/// Changed records and families in one scope, including deleted records.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScopeChanges {
    pub scope: String,
    pub records: Vec<ChangedRecord>,
    pub families: Vec<String>,
}

/// Canonical changes between two successful guarded reads.
#[derive(Debug)]
pub struct ReviewChanges {
    pub configuration_changed: bool,
    pub scopes: Vec<ScopeChanges>,
}
