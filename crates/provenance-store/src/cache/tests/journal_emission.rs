use super::super::*;
use super::fixtures::*;
use crate::publication::events_in_window;
use crate::state_store::StateStore;

fn touch(path: &camino::Utf8Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if !path.exists() {
        std::fs::write(path, b"").unwrap();
    }
}

fn mapped_families(
    layout: &crate::layout::ProvenanceLayout,
    path: &camino::Utf8Path,
) -> Vec<ProjectionFamily> {
    families_for_shard_path(layout, path)
        .into_iter()
        .map(|(family, _)| family)
        .collect()
}

#[test]
fn every_declared_shard_path_maps_back_to_its_own_family_alone() {
    let (_dir, layout, scope) = empty_layout();
    for family in ProjectionFamily::ALL {
        let shard_scope = family.is_scoped().then_some(&scope);
        let path = family.shard_path(&layout, shard_scope).unwrap();
        touch(&path);
        assert_eq!(
            mapped_families(&layout, &path),
            vec![family],
            "mapping for {path}"
        );
    }
}

#[test]
fn every_contributing_file_maps_to_the_families_it_feeds() {
    let (_dir, layout, scope) = empty_layout();

    let landings = crate::shards::ideation_landings_path(&layout, &scope);
    touch(&landings);
    assert_eq!(
        mapped_families(&layout, &landings),
        vec![
            ProjectionFamily::Contributions,
            ProjectionFamily::SynthesisPackets,
            ProjectionFamily::ProposalCards,
            ProjectionFamily::AssertionRecords,
            ProjectionFamily::Dispositions,
        ],
        "the landings overlay feeds five families"
    );

    let legacy = crate::shards::legacy_promotion_decisions_path(&layout, &scope);
    touch(&legacy);
    assert_eq!(
        mapped_families(&layout, &legacy),
        vec![ProjectionFamily::Dispositions]
    );

    let late_month = crate::shards::threads_path(&layout, &scope)
        .parent()
        .unwrap()
        .join("2031-12.jsonl");
    touch(&late_month);
    assert_eq!(
        mapped_families(&layout, &late_month),
        vec![ProjectionFamily::Messages]
    );

    let second_edges = crate::shards::edges_path(&layout)
        .parent()
        .unwrap()
        .join("edges-07.jsonl");
    touch(&second_edges);
    assert_eq!(
        mapped_families(&layout, &second_edges),
        vec![ProjectionFamily::Edges]
    );
}

#[test]
fn a_path_outside_canonical_state_maps_to_nothing() {
    let (_dir, layout, _scope) = empty_layout();
    assert!(families_for_shard_path(&layout, &layout.manifest_path()).is_empty());
    let cache_file = layout.cache_dir().join("journal/events.jsonl");
    assert!(families_for_shard_path(&layout, &cache_file).is_empty());
}

#[test]
fn an_unclaimed_canonical_path_broadcasts_as_suspect() {
    let (_dir, layout, _scope) = empty_layout();
    let stray = layout
        .scopes_dir()
        .join("default")
        .join("requirements/notes.jsonl");
    let mapped = mapped_families(&layout, &stray);
    assert_eq!(
        mapped.len(),
        ProjectionFamily::ALL.len() - 1,
        "a scoped write no domain claims hints every scoped family: {mapped:?}"
    );
    assert!(!mapped.contains(&ProjectionFamily::Edges));
}

/// The families a writer's events name after a marker sequence.
fn families_after(layout: &crate::layout::ProvenanceLayout, mark: i64) -> Vec<String> {
    let mut families: Vec<String> = events_in_window(layout, mark + 1, i64::MAX)
        .unwrap()
        .into_iter()
        .map(|event| event.family)
        .collect();
    families.sort();
    families.dedup();
    families
}

fn tail_mark(layout: &crate::layout::ProvenanceLayout) -> i64 {
    events_in_window(layout, 1, i64::MAX)
        .unwrap()
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap_or(0)
}

/// The mutation battery showed a directory-name derivation surviving: every
/// family here lives in a directory whose name is NOT the family name, so
/// an event derived from the path's parent would carry the wrong label.
#[test]
fn writer_events_carry_declared_families_not_directory_names() {
    let (_dir, layout, scope) = seeded_layout();
    let store = StateStore::new(layout.clone());

    let mark = tail_mark(&layout);
    store
        .record_requirement_reviews(
            &scope,
            vec![crate::state_store::RequirementReviewInput {
                rule_id: sid("rule_schads_pay_001"),
                requirement_id: sid("req_schads_overtime"),
                field: "statement".into(),
                before: "Overtime".into(),
                after: "Overtime pay".into(),
                changed_at: 1,
            }],
        )
        .unwrap();
    assert_eq!(
        families_after(&layout, mark),
        vec!["requirement_reviews"],
        "requirements/review.jsonl journals reviews, never `requirements`"
    );

    let mark = tail_mark(&layout);
    store
        .post_thread_message(crate::state_store::PostMessageInput {
            scope_id: scope.clone(),
            parent: provenance_core::ThreadParent {
                node_type: provenance_core::NodeType::Requirement,
                node_id: sid("req_schads_overtime"),
            },
            role: provenance_core::MessageRole::User,
            body: "Hello".into(),
        })
        .unwrap();
    let after_message = families_after(&layout, mark);
    assert!(
        after_message.contains(&"messages".to_string()),
        "threads/<month>.jsonl journals messages, never `threads` alone: {after_message:?}"
    );

    std::fs::write(layout.root().join("pay.rs"), "fn pay() {}\n").unwrap();
    let mark = tail_mark(&layout);
    store
        .materialize_implementation_binding(
            crate::state_store::MaterializeImplementationBindingInput {
                scope_id: scope.clone(),
                rule_id: sid("rule_schads_pay_001"),
                declared_by: "agent".into(),
                file: "pay.rs".into(),
                symbol: "pay".into(),
            },
        )
        .unwrap();
    assert_eq!(
        families_after(&layout, mark),
        vec!["implementation_bindings"],
        "implementations/binding.jsonl journals implementation_bindings"
    );

    let mark = tail_mark(&layout);
    store
        .materialize_verification_binding(crate::state_store::MaterializeVerificationBindingInput {
            scope_id: scope,
            rule_id: sid("rule_schads_pay_001"),
            key: "pay_examples".into(),
            method: provenance_core::VerificationMethod::Examples,
            declared_by: "agent".into(),
            file: "pay.rs".into(),
            symbol: Some("pay".into()),
        })
        .unwrap();
    assert_eq!(
        families_after(&layout, mark),
        vec!["verification_bindings"],
        "verifications/binding.jsonl journals verification_bindings"
    );
}

/// One landed batch feeds five families through one written file; every
/// hint must carry a declared family name, never the directory's.
#[test]
fn a_landed_batch_hints_every_overlay_family() {
    let (_dir, layout, scope) = seeded_layout();
    let store = StateStore::new(layout.clone());
    let mark = tail_mark(&layout);
    let batch: crate::state_store::IdeationLandingBatch =
        serde_json::from_value(serde_json::json!({
            "contributions": [{
                "schema_version": 1, "scope_id": scope.as_str(), "id": "contribution_landed",
                "target": {"artifact_type": "requirement", "artifact_id": "req_schads_overtime"},
                "participant_slot": "slot_a", "stance": "support",
                "strongest_finding": "Landed", "evidence_references": [], "material_claims": [],
                "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
                "unsupported_recommendations": [],
                "uncertainty": {"level": "low", "rationale": "R"}, "open_questions": []
            }],
            "synthesis_packets": [], "proposals": [], "assertions": [], "dispositions": []
        }))
        .unwrap();
    store.land_ideation_batch(&scope, batch, false).unwrap();
    let after_landing = families_after(&layout, mark);
    for family in [
        "assertion_records",
        "contributions",
        "dispositions",
        "proposal_cards",
        "synthesis_packets",
    ] {
        assert!(
            after_landing.contains(&family.to_string()),
            "the landings overlay must hint `{family}`: {after_landing:?}"
        );
    }
    assert!(
        !after_landing.contains(&"ideation".to_string()),
        "no event carries a directory name"
    );
}

#[test]
fn committed_writes_leave_journal_events_named_by_family() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_journal");

    let events = events_in_window(&layout, 1, i64::MAX).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.family == "sources" && event.scope == scope.as_str()),
        "a source write must journal a sources event: {events:?}"
    );
    let mut sequences: Vec<i64> = events.iter().map(|event| event.sequence).collect();
    let sorted = sequences.clone();
    sequences.sort_unstable();
    assert_eq!(sequences, sorted, "sequences allocate in order");
}

#[test]
fn a_journal_failure_degrades_to_a_lost_hint_and_never_fails_the_write() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    crate::test_probes::crash_at("writer_canonical_committed");
    let created = store.create_source(crate::state_store::CreateSourceInput {
        scope_id: scope.clone(),
        id: sid("source_survives"),
        name: "Survives".into(),
        source_type: provenance_core::SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        superseded_by: None,
        origin_thread: None,
        origin_message: None,
    });
    crate::test_probes::disarm("writer_canonical_committed");

    // The canonical write committed before the journal ran, so the caller
    // sees success; the lost hint costs the sweep one digest comparison.
    created.expect("a journal failure must not fail the committed write");
    assert_eq!(store.list_sources(&scope).unwrap().len(), 1);
    assert!(events_in_window(&layout, 1, i64::MAX).unwrap().is_empty());
}
