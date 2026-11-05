use crate::cache::gaps::{compute_gaps, GraphRecords};
use crate::cache::GapItem;
use crate::repository::Repository;
use crate::state_store::StateStore;
use provenance_core::{Message, Rule, Thread};
use std::fmt::Write;

#[derive(Debug, serde::Serialize)]
pub struct PrimeThreadView {
    pub thread: Thread,
    pub messages: Vec<Message>,
}

#[derive(Debug, serde::Serialize)]
pub struct PrimeContextView {
    pub scope_id: String,
    pub rules: Vec<Rule>,
    pub gaps: Vec<GapItem>,
    pub threads: Vec<PrimeThreadView>,
}

pub fn prime_context(
    repository: &Repository,
    include_threads: bool,
) -> anyhow::Result<PrimeContextView> {
    let store = repository.store();
    store
        .with_repository_publication(|held| prime_context_locked(repository, include_threads, held))
}

fn prime_context_locked(
    repository: &Repository,
    include_threads: bool,
    store: &StateStore,
) -> anyhow::Result<PrimeContextView> {
    let scope = repository.scope();
    let threads = if include_threads {
        let messages = store.list_messages(scope)?;
        store
            .list_threads(scope)?
            .into_iter()
            .filter(|thread| thread.status == provenance_core::ThreadStatus::Active)
            .map(|thread| PrimeThreadView {
                messages: messages
                    .iter()
                    .filter(|message| message.thread_id == thread.id)
                    .cloned()
                    .collect(),
                thread,
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(PrimeContextView {
        scope_id: scope.as_str().to_string(),
        rules: store.list_rules(scope)?.into_iter().collect(),
        gaps: compute_gaps(&GraphRecords::load(scope, store)?.graph(scope)),
        threads,
    })
}

pub fn render_prime_markdown(view: &PrimeContextView) -> String {
    let mut out = format!(
        "# Provenance Prime\n\nScope: {}\n\n## Rules\n",
        view.scope_id
    );
    for rule in &view.rules {
        let _ = writeln!(out, "- {}", rule.id.as_str());
    }
    out.push_str("\n## Gaps\n");
    if view.gaps.is_empty() {
        out.push_str("- none\n");
    }
    for gap in &view.gaps {
        let _ = writeln!(out, "- {}: {}", gap.subject(), gap.reason);
    }
    out.push_str("\n## Threads\n");
    for item in &view.threads {
        let _ = writeln!(
            out,
            "- {} on {}",
            item.thread.id.as_str(),
            item.thread.parent.node_id.as_str()
        );
        for message in &item.messages {
            let _ = writeln!(out, "  - {}: {}", message.id.as_str(), message.body);
        }
    }
    out
}
