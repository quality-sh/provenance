use provenance_core::{
    Boundary, Domain, ImplementationBinding, Question, QuestionStatus, RepoPathPrefix, Requirement,
    RequirementStatus, Resolution, ResolutionMethod, ResolutionStatus, Rule, RuleSeverity,
    RuleStatus, Source, SourceType, StableId, Topic, TopicStatus, VerificationBinding,
};
use provenance_macros::verifies;

use super::*;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

mod collaboration;
mod table;

macro_rules! define_export_test_inventory {
    (
        export { $(
            $export_variant:ident {
                record: $export_type:ty,
                field: $export_field:ident,
                path: $export_path:ident,
                meta: $export_meta:tt,
                node: [$($export_node:tt)*],
                reader: $export_reader:tt,
                id: $export_id:ident,
                loader: [$($export_loader:tt)*],
                graph: [export(
                    $export_kind:literal,
                    $export_test_kind:ident,
                    $export_fixture:ident,
                    $export_record_id:literal
                )],
                import: [$($export_import:tt)*],
                catalog: [$($export_catalog:tt)*],
                route: [$($export_route:tt)*]
            };
        )* }
        canonical { $($canonical:tt)* }
        bindings { $(
            $binding_variant:ident {
                record: $binding_type:ty,
                field: $binding_field:ident,
                path: $binding_path:ident,
                meta: $binding_meta:tt,
                node: [$($binding_node:tt)*],
                reader: $binding_reader:tt,
                id: $binding_id:ident,
                loader: [$($binding_loader:tt)*],
                graph: [$binding_phase:ident(
                    $binding_kind:literal,
                    $binding_test_kind:ident,
                    $binding_fixture:ident,
                    $binding_record_id:literal
                )],
                import: [$($binding_import:tt)*],
                catalog: [$($binding_catalog:tt)*],
                route: [$($binding_route:tt)*]
            };
        )* }
        internal { $($internal:tt)* }
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum RecordFamily {
            $($export_test_kind,)*
            $($binding_test_kind,)*
        }

        fn all_families() -> Vec<RecordFamily> {
            vec![
                $(RecordFamily::$export_test_kind,)*
                $(RecordFamily::$binding_test_kind,)*
            ]
        }

        const fn record_id(family: RecordFamily) -> &'static str {
            match family {
                $(RecordFamily::$export_test_kind => $export_record_id,)*
                $(RecordFamily::$binding_test_kind => $binding_record_id,)*
            }
        }

        fn graph_in_scope(scope: &ScopeId, populated: &[RecordFamily]) -> GraphExport {
            let holds = |family: RecordFamily| populated.contains(&family);
            GraphExport {
                schema_version: 1,
                scope: Scope {
                    id: scope.clone(),
                    path_prefix: RepoPathPrefix::new("."),
                },
                $($export_field: holds(RecordFamily::$export_test_kind)
                    .then(|| $export_fixture(scope)).into_iter().collect(),)*
                $($binding_field: holds(RecordFamily::$binding_test_kind)
                    .then(|| $binding_fixture(scope)).into_iter().collect(),)*
            }
        }

        fn move_family_to_scope(
            graph: &mut GraphExport,
            family: RecordFamily,
            scope: &ScopeId,
        ) {
            match family {
                $(RecordFamily::$export_test_kind => {
                    for record in &mut graph.$export_field {
                        record.scope_id = scope.clone();
                    }
                },)*
                $(RecordFamily::$binding_test_kind => {
                    for record in &mut graph.$binding_field {
                        record.scope_id = scope.clone();
                    }
                },)*
            }
        }

        fn family_count_of(graph: &GraphExport) -> usize {
            let GraphExport {
                schema_version: _,
                scope: _,
                $($export_field,)*
                $($binding_field,)*
            } = graph;
            let counts = [
                $($export_field.len(),)*
                $($binding_field.len(),)*
            ];
            assert!(
                counts.iter().all(|count| *count == 1),
                "the fixture must hold one record of each family, got {counts:?}"
            );
            counts.len()
        }
    };
}

crate::cache::family_table::record_family_rows!(define_export_test_inventory);

fn stable_id(family: RecordFamily) -> StableId {
    StableId::new(record_id(family)).unwrap()
}

fn source_record(scope: &ScopeId) -> Source {
    Source {
        created: None,
        updated: None,
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::Source),
        declared_by: None,
        declaration_address: None,

        name: "Pinned source".into(),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

fn domain_record(scope: &ScopeId) -> Domain {
    Domain {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::Domain),
        name: "Pinned domain".into(),
        description: None,
        color: None,
    }
}

fn requirement_record(scope: &ScopeId) -> Requirement {
    Requirement {
        created: None,
        updated: None,
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::Requirement),
        declared_by: None,
        declaration_address: None,

        statement: "Pinned requirement".into(),
        description: None,
        fog: None,
        status: RequirementStatus::Active,
        domain_id: None,
        source_refs: Vec::new(),
        refines: None,
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn boundary_record(scope: &ScopeId) -> Boundary {
    Boundary {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::Boundary),
        requirement_id: stable_id(RecordFamily::Requirement),
        statement: "Pinned boundary".into(),
        source_ref: None,
    }
}

fn topic_record(scope: &ScopeId) -> Topic {
    Topic {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::Topic),
        requirement_id: stable_id(RecordFamily::Requirement),
        title: "Pinned topic".into(),
        status: TopicStatus::Open,
        claimed_by: None,
        claimed_at: None,
        links: Vec::new(),
    }
}

fn question_record(scope: &ScopeId) -> Question {
    Question {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::Question),
        topic_id: stable_id(RecordFamily::Topic),
        requirement_id: stable_id(RecordFamily::Requirement),
        question: "Pinned question?".into(),
        resolution_method: ResolutionMethod::Grill,
        status: QuestionStatus::Open,
        claimed_by: None,
        claimed_at: None,
        answer: None,
        links: Vec::new(),
        contradicts: None,
        resolution_id: None,
    }
}

fn resolution_record(scope: &ScopeId) -> Resolution {
    Resolution {
        created: None,
        updated: None,
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::Resolution),
        title: "Pinned resolution".into(),
        position: "Pinned position".into(),
        rationale: "Pinned rationale".into(),
        status: ResolutionStatus::Approved,
        context: None,
        enforcement: None,
        confidence: None,
        inputs: Vec::new(),
        made_by: None,
        approved_by: None,
        approved_at: None,
        review_on: None,
        requirement_ids: Vec::new(),
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

fn rule_record(scope: &ScopeId) -> Rule {
    Rule {
        created: None,
        updated: None,
        archived_in_commit: None,
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::Rule),
        declared_by: None,
        declaration_address: None,

        name: None,
        description: None,
        statement: "Pinned rule".into(),
        status: RuleStatus::Active,
        severity: RuleSeverity::Medium,
        source_document: None,
        source_section: None,
        requirement_ids: Vec::new(),
        resolution_ids: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

fn verification_binding_record(scope: &ScopeId) -> VerificationBinding {
    VerificationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::VerificationBinding),
        rule_id: stable_id(RecordFamily::Rule),
        key: "pinned-check".into(),
        method: provenance_core::VerificationMethod::Examples,
        declared_by: "ci://typescript".into(),

        file: "tests/pinned.test.ts".into(),
        symbol: Some("pinned check".into()),
    }
}

fn implementation_binding_record(scope: &ScopeId) -> ImplementationBinding {
    ImplementationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: stable_id(RecordFamily::ImplementationBinding),
        rule_id: stable_id(RecordFamily::Rule),
        declared_by: "spec://typescript/pinned".into(),

        file: "src/pinned.ts".into(),
        symbol: "pinnedImplementation".into(),
    }
}

#[test]
#[verifies("rule_pinned_scope_ownership", exhaustion)]
fn rejects_a_stray_record_of_every_family() {
    let claimed = ScopeId::new("default").unwrap();
    let elsewhere = ScopeId::new("other").unwrap();
    let families = all_families();

    assert_eq!(
        family_count_of(&graph_in_scope(&claimed, &families)),
        families.len(),
        "every record family of GraphExport must be covered by RecordFamily"
    );

    for &family in &families {
        let mut graph = graph_in_scope(&claimed, &families);
        move_family_to_scope(&mut graph, family, &elsewhere);
        let Err(GraphReferenceError::Incomplete { detail }) =
            validate_scope_ownership(&graph, &claimed)
        else {
            panic!("a pinned graph holding a {family:?} record from scope 'other' was accepted");
        };
        assert!(
            detail.contains(record_id(family)),
            "the refusal must name the offending {family:?} record, got: {detail}"
        );
        assert!(
            detail.contains("'other'") && detail.contains("'default'"),
            "the refusal must name both the record's scope and the claimed scope, got: {detail}"
        );
    }
}

#[test]
#[verifies("rule_pinned_scope_ownership", property)]
fn accepts_any_graph_whose_records_all_sit_in_the_claimed_scope() {
    // The property: a graph passes whenever no record carries a scope id
    // other than the claimed one. Stated without reference to the record
    // families, so which families are populated must not matter. The
    // generator walks every subset of the families, twice over, once per
    // claimed scope id.
    let families = all_families();
    for scope_name in ["default", "team-b"] {
        let claimed = ScopeId::new(scope_name).unwrap();
        for mask in 0u32..(1u32 << families.len()) {
            let populated: Vec<RecordFamily> = families
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1u32 << index) != 0)
                .map(|(_, family)| *family)
                .collect();
            let graph = graph_in_scope(&claimed, &populated);
            assert!(
                validate_scope_ownership(&graph, &claimed).is_ok(),
                "scope '{scope_name}' rejected a graph whose records all sit in it: {populated:?}"
            );
        }
    }
}

#[test]
fn graph_digest_ignores_record_stamps() {
    let scope = ScopeId::new("default").unwrap();
    let families = all_families();
    let graph = graph_in_scope(&scope, &families);
    let mut stamped = graph.clone();
    let created = provenance_core::Stamp {
        commit: "a".repeat(40),
        at: "2026-09-12T00:00:00Z".into(),
    };
    let updated = provenance_core::Stamp {
        commit: "b".repeat(40),
        at: "2026-09-12T01:00:00Z".into(),
    };
    stamped.sources[0].created = Some(created.clone());
    stamped.sources[0].updated = Some(updated.clone());
    stamped.requirements[0].created = Some(created.clone());
    stamped.requirements[0].updated = Some(updated.clone());
    stamped.resolutions[0].created = Some(created.clone());
    stamped.resolutions[0].updated = Some(updated.clone());
    stamped.rules[0].created = Some(created);
    stamped.rules[0].updated = Some(updated);

    assert_eq!(graph, stamped, "record equality must ignore stamps");
    assert_eq!(
        crate::graph_reference::graph_digest(&graph).unwrap(),
        crate::graph_reference::graph_digest(&stamped).unwrap()
    );
}

mod review;
