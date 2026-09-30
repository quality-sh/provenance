use super::{BudgetKind, FamilyGroup, FamilyMeta, ProjectionFamily, FAMILIES};
use provenance_core::NodeType;

fn expected(
    identity: (FamilyGroup, &'static str),
    shard_suffix: &'static str,
    node_type: Option<NodeType>,
    graph_field: Option<&'static str>,
    route_order: Option<u16>,
    budget: BudgetKind,
    catalog_operations: &'static [&'static str],
) -> FamilyMeta {
    let (group, table_name) = identity;
    FamilyMeta {
        group,
        table_name,
        shard_suffix,
        node_type,
        graph_field,
        route_order,
        budget,
        terminal_statuses: &[],
        catalog_operations,
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn every_family_keeps_its_independent_descriptor() {
    use BudgetKind::{NotImported, Record, Resource, Unchecked};
    use FamilyGroup::{Binding, Canonical, Export, Internal};
    let cases = [
        (
            ProjectionFamily::Sources,
            expected(
                (Export, "sources"),
                "sources/source.jsonl",
                Some(NodeType::Source),
                Some("sources"),
                Some(10),
                Resource,
                &["list-sources", "page-sources", "get-source"],
            ),
        ),
        (
            ProjectionFamily::Domains,
            expected(
                (Export, "domains"),
                "domains/domain.jsonl",
                Some(NodeType::Domain),
                Some("domains"),
                Some(50),
                Resource,
                &["list-domains", "page-domains", "get-domain"],
            ),
        ),
        (
            ProjectionFamily::Requirements,
            expected(
                (Export, "requirements"),
                "requirements/req.jsonl",
                Some(NodeType::Requirement),
                Some("requirements"),
                Some(20),
                Resource,
                &["list-requirements", "page-requirements", "get-requirement"],
            ),
        ),
        (
            ProjectionFamily::Boundaries,
            expected(
                (Export, "boundaries"),
                "boundaries/boundary.jsonl",
                Some(NodeType::Boundary),
                Some("boundaries"),
                Some(60),
                Resource,
                &["list-boundaries", "page-boundaries", "get-boundary"],
            ),
        ),
        (
            ProjectionFamily::Topics,
            expected(
                (Export, "topics"),
                "topics/topic.jsonl",
                Some(NodeType::Topic),
                Some("topics"),
                Some(70),
                Resource,
                &["list-topics", "page-topics", "get-topic"],
            ),
        ),
        (
            ProjectionFamily::Questions,
            expected(
                (Export, "questions"),
                "questions/question.jsonl",
                Some(NodeType::Question),
                Some("questions"),
                Some(80),
                Resource,
                &["list-questions", "page-questions", "get-question"],
            ),
        ),
        (
            ProjectionFamily::Resolutions,
            expected(
                (Export, "resolutions"),
                "resolutions/res.jsonl",
                Some(NodeType::Resolution),
                Some("resolutions"),
                Some(30),
                Resource,
                &["list-resolutions", "page-resolutions", "get-resolution"],
            ),
        ),
        (
            ProjectionFamily::Rules,
            expected(
                (Export, "rules"),
                "rules/rule.jsonl",
                Some(NodeType::Rule),
                Some("rules"),
                Some(40),
                Resource,
                &["list-rules", "page-rules", "get-rule"],
            ),
        ),
        (
            ProjectionFamily::Threads,
            expected(
                (Canonical, "threads"),
                "threads/threads.jsonl",
                None,
                None,
                Some(140),
                Record,
                &[
                    "list-discussion-containers",
                    "page-discussion-containers",
                    "get-discussion-container",
                ],
            ),
        ),
        (
            ProjectionFamily::Messages,
            expected(
                (Canonical, "messages"),
                "threads/2026-07.jsonl",
                None,
                None,
                Some(150),
                Record,
                &["list-messages", "page-messages", "get-message"],
            ),
        ),
        (
            ProjectionFamily::Contributions,
            expected(
                (Canonical, "contributions"),
                "ideation/contributions.jsonl",
                None,
                None,
                Some(90),
                Resource,
                &[
                    "list-contributions",
                    "page-contributions",
                    "get-contribution",
                ],
            ),
        ),
        (
            ProjectionFamily::SynthesisPackets,
            expected(
                (Canonical, "synthesis_packets"),
                "ideation/synthesis_packets.jsonl",
                None,
                None,
                Some(100),
                Resource,
                &[
                    "list-synthesis-packets",
                    "page-synthesis-packets",
                    "get-synthesis-packet",
                ],
            ),
        ),
        (
            ProjectionFamily::ProposalCards,
            expected(
                (Canonical, "proposal_cards"),
                "ideation/proposal_cards.jsonl",
                None,
                None,
                Some(110),
                Resource,
                &["list-proposals", "page-proposals", "get-proposal"],
            ),
        ),
        (
            ProjectionFamily::AssertionRecords,
            expected(
                (Canonical, "assertion_records"),
                "ideation/assertions.jsonl",
                None,
                None,
                Some(160),
                Resource,
                &["list-assertions", "page-assertions", "get-assertion"],
            ),
        ),
        (
            ProjectionFamily::Dispositions,
            expected(
                (Canonical, "dispositions"),
                "ideation/dispositions.jsonl",
                None,
                None,
                Some(170),
                Resource,
                &["list-dispositions", "page-dispositions", "get-disposition"],
            ),
        ),
        (
            ProjectionFamily::ImplementationBindings,
            expected(
                (Binding, "implementation_bindings"),
                "implementations/binding.jsonl",
                None,
                Some("implementation_bindings"),
                None,
                Unchecked,
                &[],
            ),
        ),
        (
            ProjectionFamily::VerificationBindings,
            expected(
                (Binding, "verification_bindings"),
                "verifications/binding.jsonl",
                None,
                Some("verification_bindings"),
                Some(130),
                Resource,
                &[
                    "list-verification-bindings",
                    "page-verification-bindings",
                    "get-verification-binding",
                ],
            ),
        ),
        (
            ProjectionFamily::RequirementReviews,
            expected(
                (Internal, "requirement_reviews"),
                "requirements/review.jsonl",
                None,
                None,
                None,
                NotImported,
                &[],
            ),
        ),
        (
            ProjectionFamily::ReviewJournal,
            expected(
                (Internal, "review_journal"),
                "review/journal",
                None,
                None,
                None,
                NotImported,
                &[],
            ),
        ),
    ];

    assert_eq!(ProjectionFamily::ALL.len(), cases.len());
    assert_eq!(FAMILIES.len(), cases.len());
    let layout = crate::layout::ProvenanceLayout::new("/repo");
    let scope = provenance_core::ScopeId::new("default").unwrap();
    for (family, descriptor) in cases {
        let descriptor = FamilyMeta {
            terminal_statuses: family.meta().terminal_statuses,
            ..descriptor
        };
        assert_eq!(*family.meta(), descriptor, "descriptor for {family:?}");
        assert_eq!(
            family.shard_path(&layout, &scope),
            layout
                .scopes_dir()
                .join("default")
                .join(descriptor.shard_suffix),
            "shard path for {family:?}"
        );
    }
}

#[test]
fn terminal_predicate_follows_each_family_row() {
    let predicate = super::terminal_and_dead_predicate("candidate.kind", "candidate.id", "?1");

    for family in FAMILIES.iter().filter(|family| family.node_type.is_some()) {
        let node_type = family.node_type.unwrap().as_str();
        let marker = format!("candidate.kind = '{node_type}'");
        assert_eq!(
            predicate.matches(&marker).count(),
            usize::from(!family.terminal_statuses.is_empty()),
            "predicate clause for {}",
            family.table_name
        );
        for status in family.terminal_statuses {
            assert!(
                predicate.contains(&format!("'{status}'")),
                "status for {}",
                family.table_name
            );
        }
    }
}

#[test]
fn catalog_registers_each_family_operation() {
    let registered = crate::operations::catalog::registered_operation_names_for_test();
    for family in ProjectionFamily::ALL {
        for operation in family.catalog_operation_names() {
            assert!(
                registered.contains(operation),
                "{} operation {operation} is absent from the catalog",
                family.family_name()
            );
        }
    }
}
