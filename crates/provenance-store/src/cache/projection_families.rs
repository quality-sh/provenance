//! The one table of projection families.
//!
//! `PROJECTION_FAMILIES` names every family the `SQLite` projection stores.
//! The digest assembler, the materialize loaders, the byte-verify sweep,
//! and the write journal all read this one table, so adding a family
//! updates coverage, invalidation, and verification together. A family
//! without a row here cannot be stored or stamped.

use crate::layout::ProvenanceLayout;
use crate::shards;
use crate::state_store::StateStore;
use camino::Utf8PathBuf;
use provenance_core::ScopeId;

/// One family the `SQLite` projection stores.
pub struct ProjectionFamily {
    /// Family name used by the journal and `projection_family_digests`.
    pub name: &'static str,
    /// `SQLite` table the family materializes into.
    pub table: &'static str,
    /// Whether the family is one global shard (edges) rather than a
    /// per-scope shard.
    pub global: bool,
    /// Canonical shard path for the family.
    pub shard: fn(&ProvenanceLayout, &ScopeId) -> Utf8PathBuf,
    /// Canonical records behind the family, as serde values in stored
    /// record shape.
    pub records: fn(&StateStore, &ScopeId) -> anyhow::Result<Vec<serde_json::Value>>,
}

/// Declares the fn-pointer record loader one table row binds.
macro_rules! loader {
    ($name:ident, $reader:expr) => {
        fn $name(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<serde_json::Value>> {
            let records: Vec<_> = $reader(store, scope)?;
            records
                .iter()
                .map(|record| serde_json::to_value(record).map_err(anyhow::Error::from))
                .collect()
        }
    };
}

loader!(load_sources, |s: &StateStore, sc: &ScopeId| s
    .list_sources(sc));
loader!(load_domains, |s: &StateStore, sc: &ScopeId| s
    .list_domains(sc));
loader!(load_requirements, |s: &StateStore, sc: &ScopeId| s
    .list_requirements(sc));
loader!(load_boundaries, |s: &StateStore, sc: &ScopeId| s
    .list_boundaries(sc));
loader!(load_topics, |s: &StateStore, sc: &ScopeId| s
    .list_topics(sc));
loader!(load_questions, |s: &StateStore, sc: &ScopeId| s
    .list_questions(sc));
loader!(load_resolutions, |s: &StateStore, sc: &ScopeId| s
    .list_resolutions(sc));
loader!(load_rules, |s: &StateStore, sc: &ScopeId| s.list_rules(sc));
loader!(load_messages, |s: &StateStore, sc: &ScopeId| s
    .list_messages(sc));
loader!(load_threads, |s: &StateStore, sc: &ScopeId| s
    .list_threads(sc));
loader!(load_contributions, |s: &StateStore, sc: &ScopeId| s
    .list_contributions(sc));
loader!(load_synthesis_packets, |s: &StateStore, sc: &ScopeId| s
    .list_synthesis_packets(sc));
loader!(load_proposal_cards, |s: &StateStore, sc: &ScopeId| s
    .list_proposal_cards(sc));
loader!(load_assertion_records, |s: &StateStore, sc: &ScopeId| s
    .list_assertion_records(sc));
loader!(load_dispositions, |s: &StateStore, sc: &ScopeId| s
    .list_dispositions(sc));
loader!(
    load_implementation_bindings,
    |s: &StateStore, sc: &ScopeId| s.list_implementation_bindings(sc)
);
loader!(
    load_verification_bindings,
    |s: &StateStore, sc: &ScopeId| s.list_verification_bindings(sc)
);
loader!(load_requirement_reviews, |s: &StateStore, sc: &ScopeId| s
    .open_requirement_reviews(sc));

fn all_edges(store: &StateStore, _scope: &ScopeId) -> anyhow::Result<Vec<serde_json::Value>> {
    store
        .list_edges()?
        .iter()
        .map(|edge| serde_json::to_value(edge).map_err(anyhow::Error::from))
        .collect()
}

fn edges_shard(layout: &ProvenanceLayout, _scope: &ScopeId) -> Utf8PathBuf {
    shards::edges_path(layout)
}

/// Every family the `SQLite` projection stores, in stamped order.
pub static PROJECTION_FAMILIES: &[ProjectionFamily] = &[
    family("sources", "sources", shards::sources_path, load_sources),
    family("domains", "domains", shards::domains_path, load_domains),
    family(
        "requirements",
        "requirements",
        shards::requirements_path,
        load_requirements,
    ),
    family(
        "boundaries",
        "boundaries",
        shards::boundaries_path,
        load_boundaries,
    ),
    family("topics", "topics", shards::topics_path, load_topics),
    family(
        "questions",
        "questions",
        shards::questions_path,
        load_questions,
    ),
    global_family("edges", "edges", edges_shard, all_edges),
    family(
        "resolutions",
        "resolutions",
        shards::resolutions_path,
        load_resolutions,
    ),
    family("rules", "rules", shards::rules_path, load_rules),
    family("messages", "messages", shards::messages_path, load_messages),
    family("threads", "threads", shards::threads_path, load_threads),
    family(
        "contributions",
        "contributions",
        shards::contributions_path,
        load_contributions,
    ),
    family(
        "synthesis_packets",
        "synthesis_packets",
        shards::synthesis_packets_path,
        load_synthesis_packets,
    ),
    family(
        "proposal_cards",
        "proposal_cards",
        shards::proposal_cards_path,
        load_proposal_cards,
    ),
    family(
        "assertion_records",
        "assertion_records",
        shards::assertion_records_path,
        load_assertion_records,
    ),
    family(
        "dispositions",
        "dispositions",
        shards::dispositions_path,
        load_dispositions,
    ),
    family(
        "implementation_bindings",
        "implementation_bindings",
        shards::implementation_bindings_path,
        load_implementation_bindings,
    ),
    family(
        "verification_bindings",
        "verification_bindings",
        shards::verification_bindings_path,
        load_verification_bindings,
    ),
    family(
        "requirement_reviews",
        "requirement_reviews",
        shards::requirement_reviews_path,
        load_requirement_reviews,
    ),
];

const fn family(
    name: &'static str,
    table: &'static str,
    shard: fn(&ProvenanceLayout, &ScopeId) -> Utf8PathBuf,
    records: fn(&StateStore, &ScopeId) -> anyhow::Result<Vec<serde_json::Value>>,
) -> ProjectionFamily {
    ProjectionFamily {
        name,
        table,
        global: false,
        shard,
        records,
    }
}

const fn global_family(
    name: &'static str,
    table: &'static str,
    shard: fn(&ProvenanceLayout, &ScopeId) -> Utf8PathBuf,
    records: fn(&StateStore, &ScopeId) -> anyhow::Result<Vec<serde_json::Value>>,
) -> ProjectionFamily {
    ProjectionFamily {
        name,
        table,
        global: true,
        shard,
        records,
    }
}

pub fn family_named(name: &str) -> Option<&'static ProjectionFamily> {
    PROJECTION_FAMILIES
        .iter()
        .find(|family| family.name == name)
}

/// Canonical records behind one family across every named scope.
///
/// A global family is read once; a scoped family is read per scope, in
/// the order the scopes are named.
pub fn family_records(
    family: &ProjectionFamily,
    store: &StateStore,
    scopes: &[provenance_core::Scope],
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut records = Vec::new();
    if family.global {
        if let Some(scope) = scopes.first() {
            records.extend((family.records)(store, &scope.id)?);
        }
    } else {
        for scope in scopes {
            records.extend((family.records)(store, &scope.id)?);
        }
    }
    Ok(records)
}
