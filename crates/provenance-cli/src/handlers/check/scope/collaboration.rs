use crate::handlers::check::index::CheckIndex;
use crate::handlers::check::references::{check_scoped_reference, node_type_name};
use crate::store::ScopeSnapshot;
use provenance_core::{Message, ScopeId, Thread};
use std::collections::BTreeSet;

pub(super) struct Records<'a> {
    threads: &'a [Thread],
    messages: &'a [Message],
}

impl<'a> Records<'a> {
    pub(super) const fn load(snapshot: &'a ScopeSnapshot) -> Self {
        Self {
            threads: &snapshot.threads,
            messages: &snapshot.messages,
        }
    }

    pub(super) fn validate_scope_ownership(
        &self,
        manifest_scopes: &BTreeSet<String>,
        loaded_scope_id: &ScopeId,
        findings: &mut Vec<String>,
    ) {
        macro_rules! check_records {
            ($records:expr, $record_type:literal) => {
                for record in $records {
                    super::check_scope_ownership(
                        loaded_scope_id,
                        &record.scope_id,
                        $record_type,
                        &record.id,
                        findings,
                    );
                }
            };
        }

        for thread in self.threads {
            if manifest_scopes.contains(thread.scope_id.as_str()) {
                super::check_scope_ownership(
                    loaded_scope_id,
                    &thread.scope_id,
                    "thread",
                    &thread.id,
                    findings,
                );
            }
        }
        check_records!(self.messages, "message");
    }

    pub(super) fn add_to(&self, index: &mut CheckIndex) {
        for thread in self.threads {
            index.add_node(&thread.scope_id, "thread", &thread.id);
        }
        for message in self.messages {
            index.add_node(&message.scope_id, "message", &message.id);
        }
    }

    pub(super) fn validate(
        &self,
        index: &CheckIndex,
        manifest_scopes: &BTreeSet<String>,
        scope_id: &ScopeId,
        dangling: &mut Vec<String>,
    ) {
        for thread in self.threads {
            let owner = format!("thread {}", thread.id.as_str());
            if !manifest_scopes.contains(thread.scope_id.as_str()) {
                dangling.push(format!(
                    "{owner} is in unknown scope {}",
                    thread.scope_id.as_str()
                ));
            }
            check_scoped_reference(
                index,
                dangling,
                scope_id,
                &owner,
                "parent",
                node_type_name(thread.parent.node_type),
                &thread.parent.node_id,
            );
        }
        for message in self.messages {
            check_scoped_reference(
                index,
                dangling,
                scope_id,
                &format!("message {}", message.id.as_str()),
                "thread",
                "thread",
                &message.thread_id,
            );
        }
    }
}
