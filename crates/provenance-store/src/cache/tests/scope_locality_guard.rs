//! The scope-locality invariant, checked against real reads.
//!
//! Catch-up hashes one unit per scope and one global unit, and trusts that
//! a scope's rows derive only from files inside those units. That trust is
//! not a comment: the reader choke points record every canonical path they
//! open, and these tests assert the recorded set lies inside the hashed
//! units — the scope directory of the scope being derived, or the global
//! unit. A reader that reaches into another scope, or outside `state/`,
//! goes red here before it can go stale in the projection.

use super::super::*;
use super::fixtures::*;
use crate::state_store::StateStore;
use provenance_core::ScopeId;
use std::collections::BTreeSet;

/// Where a recorded read sits: inside a scope, in the global unit, or
/// outside canonical state altogether.
enum Locality {
    Scope(String),
    Global,
    Outside,
}

fn locality(path: &str) -> Locality {
    let Some((_, inside)) = path.split_once("/.provenance/state/") else {
        return Locality::Outside;
    };
    inside
        .strip_prefix("scopes/")
        .map_or(Locality::Global, |rest| {
            Locality::Scope(rest.split('/').next().unwrap_or("").to_string())
        })
}

fn two_scope_layout() -> (
    tempfile::TempDir,
    crate::layout::ProvenanceLayout,
    ScopeId,
    ScopeId,
) {
    let (dir, layout, first) = seeded_layout();
    let second = ScopeId::new("second").unwrap();
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.scopes.push(provenance_core::Scope {
        id: second.clone(),
        path_prefix: provenance_core::RepoPathPrefix::new("second"),
    });
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let requirements = crate::shards::requirements_path(&layout, &second);
    std::fs::create_dir_all(requirements.parent().unwrap()).unwrap();
    std::fs::write(
        &requirements,
        format!(
            "{}\n",
            serde_json::json!({"schema_version": 1, "scope_id": "second",
                "id": "req_second", "statement": "Second", "status": "active"})
        ),
    )
    .unwrap();
    (dir, layout, first, second)
}

#[tokio::test]
async fn a_rebuild_reads_only_inside_the_hashed_units() {
    let (_dir, layout, first, second) = two_scope_layout();
    crate::test_probes::start_recording_reads();
    materialize_state(&layout).await.unwrap();
    let reads = crate::test_probes::take_recorded_reads();
    assert!(!reads.is_empty(), "the loaders must record their reads");

    let scopes: BTreeSet<String> = [first.as_str(), second.as_str()]
        .into_iter()
        .map(str::to_string)
        .collect();
    for path in &reads {
        match locality(path) {
            Locality::Scope(scope) => assert!(
                scopes.contains(&scope),
                "read outside every manifest scope: {path}"
            ),
            Locality::Global => {}
            Locality::Outside => panic!("read outside canonical state: {path}"),
        }
    }
}

#[test]
fn each_scoped_family_derives_from_its_own_scope_and_the_global_unit_only() {
    let (_dir, layout, first, second) = two_scope_layout();
    let store = StateStore::new(layout);
    let mut recorded_any = false;
    for scope in [&first, &second] {
        for family in ProjectionFamily::ALL.into_iter().filter(|f| f.is_scoped()) {
            crate::test_probes::start_recording_reads();
            family.canonical_records(&store, Some(scope)).unwrap();
            let reads = crate::test_probes::take_recorded_reads();
            // A family whose directory does not exist yet reads nothing; the
            // month-shard discovery, for one, opens no file then.
            recorded_any |= !reads.is_empty();
            for path in &reads {
                match locality(path) {
                    Locality::Scope(read_scope) => assert_eq!(
                        &read_scope,
                        scope.as_str(),
                        "family `{}` of scope `{}` read another scope: {path}",
                        family.family_name(),
                        scope.as_str()
                    ),
                    Locality::Global => {}
                    Locality::Outside => panic!(
                        "family `{}` read outside canonical state: {path}",
                        family.family_name()
                    ),
                }
            }
        }
    }
    assert!(recorded_any, "the derivations must record their reads");
}

#[test]
fn the_edges_family_derives_from_the_global_unit_only() {
    let (_dir, layout, _first, _second) = two_scope_layout();
    let store = StateStore::new(layout);
    crate::test_probes::start_recording_reads();
    ProjectionFamily::Edges
        .canonical_records(&store, None)
        .unwrap();
    let reads = crate::test_probes::take_recorded_reads();
    assert!(!reads.is_empty());
    for path in &reads {
        assert!(
            matches!(locality(path), Locality::Global),
            "edges read outside the global unit: {path}"
        );
    }
}
