//! Identities changed by one page write, captured while publication excludes other writers.
use super::{ChangedRecord, ReviewChanges, ReviewStateSnapshot};
use std::collections::BTreeSet;

impl ReviewStateSnapshot {
    /// Includes the document identities whose review summaries changed with the saved facts.
    pub fn page_changes_since(&self, previous: &Self) -> ReviewChanges {
        let mut changes = self.changes_since(previous);
        for scope in &mut changes.scopes {
            let mut related = BTreeSet::new();
            for record in &scope.records {
                let key = (scope.scope.clone(), record.kind.clone(), record.id.clone());
                for value in [self.records.get(&key), previous.records.get(&key)]
                    .into_iter()
                    .flatten()
                {
                    if let Some(proposal) = value["proposal_id"].as_str() {
                        let proposal_key = (
                            scope.scope.clone(),
                            "proposal_cards".to_owned(),
                            proposal.to_owned(),
                        );
                        if let Some(target) = self
                            .records
                            .get(&proposal_key)
                            .or_else(|| previous.records.get(&proposal_key))
                            .and_then(|proposal| {
                                proposal.pointer("/traceability/target/artifact_id")
                            })
                            .and_then(serde_json::Value::as_str)
                        {
                            related.insert(target.to_owned());
                        }
                    }
                    for pointer in [
                        "/canonical_artifact/artifact_id",
                        "/traceability/target/artifact_id",
                        "/parent/node_id",
                        "/thread_id",
                    ] {
                        if let Some(id) = value.pointer(pointer).and_then(serde_json::Value::as_str)
                        {
                            related.insert(id.to_owned());
                        }
                    }
                }
            }
            for id in related {
                if !scope.records.iter().any(|record| record.id == id) {
                    scope.records.push(ChangedRecord {
                        kind: "review".to_owned(),
                        id,
                    });
                }
            }
        }
        changes
    }
}
