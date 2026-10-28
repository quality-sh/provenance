use super::{ChangedRecord, ReviewChanges, ReviewStateSnapshot, ScopeChanges};
use std::collections::{BTreeMap, BTreeSet};

impl ReviewStateSnapshot {
    /// Compares canonical content and identities, including removed scopes and records.
    pub fn changes_since(&self, previous: &Self) -> ReviewChanges {
        let keys: BTreeSet<_> = self.records.keys().chain(previous.records.keys()).collect();
        let mut scopes: BTreeMap<String, (Vec<ChangedRecord>, BTreeSet<String>)> = BTreeMap::new();
        for key @ (scope, kind, id) in keys {
            if self.records.get(key) == previous.records.get(key) {
                continue;
            }
            let (records, families) = scopes.entry(scope.clone()).or_default();
            records.push(ChangedRecord {
                kind: kind.clone(),
                id: id.clone(),
            });
            families.insert(kind.clone());
        }
        ReviewChanges {
            configuration_changed: self.manifest != previous.manifest
                || self.settings != previous.settings,
            scopes: scopes
                .into_iter()
                .map(|(scope, (records, families))| ScopeChanges {
                    scope,
                    records,
                    families: families.into_iter().collect(),
                })
                .collect(),
        }
    }
}
