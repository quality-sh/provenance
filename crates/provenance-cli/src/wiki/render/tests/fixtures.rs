use crate::wiki::links::LinkResolver;
use crate::wiki::model::{
    CodeScan, CorpusCounts, DecisionSection, DomainGroup, DomainIndexPage, DomainState,
    EvidenceThread, FieldNote, GapKind, GapNotice, HomepageDomain, ImplementationBinding,
    InputCitation, LineageEntry, PageId, PageKind, PageLink, RecordKind, RequirementPage,
    ResolutionPage, RuleCard, RulePage, ScopeIndexPage, SearchEntry, SourceCitation, SourcePage,
    VerificationSite, WikiCorpus,
};
use provenance_core::coverage::{CoverageReport, CoverageScan, ScannedFile};
use provenance_core::{
    MessageRole, NodeType, RequirementStatus, ResolutionInputType, ResolutionStatus, RuleSeverity,
    RuleStatus, SourceType, ThreadStatus,
};
use std::fmt::Write as _;

pub(super) use super::fixtures_discovery::{decisions_fixture, search_fixture, unfinished_fixture};

pub(super) const REMOTE: &str = "git@github.com:exampleorg/ex-api.git";
/// The fixture scan commit; binding links pin to it as real scans do.
pub(super) const SCAN_COMMIT: &str = "9f2c1ab4e5f6";

fn resolver() -> LinkResolver {
    let report = CoverageScan {
        report: CoverageReport::new(
            Some(SCAN_COMMIT.to_string()),
            2,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        scanned_files: vec![
            ScannedFile {
                file_path: "src/UseCase.php".into(),
                content: (1..=240).fold(String::new(), |mut content, line| {
                    writeln!(content, "source line {line}").unwrap();
                    content
                }),
            },
            ScannedFile {
                file_path: "tests/UseCaseTest.php".into(),
                content: String::new(),
            },
        ],
    };
    LinkResolver::new(Some(REMOTE)).with_coverage(&report)
}

pub(super) fn link(kind: PageKind, id: &str, title: &str) -> PageLink {
    let kind = match kind {
        PageKind::Requirement => RecordKind::Requirement,
        PageKind::Resolution => RecordKind::Resolution,
        PageKind::Rule => RecordKind::Rule,
        PageKind::Source => RecordKind::Source,
        PageKind::ScopeIndex
        | PageKind::DomainIndex
        | PageKind::SearchIndex
        | PageKind::DecisionIndex
        | PageKind::Unfinished => {
            panic!("singleton pages are not record links")
        }
    };
    PageLink {
        target: PageId::new(kind, id),
        title: title.to_string(),
    }
}

pub(super) fn colliding_requirement_links() -> Vec<PageLink> {
    vec![
        link(
            PageKind::Requirement,
            "req_sah_participant_budget_summary_shall_pro",
            "Participant budget summary shall pro-rate services",
        ),
        link(
            PageKind::Requirement,
            "req_sah_participant_budget_summary_shall_pro_2",
            "Participant budget summary shall pro-rate services",
        ),
    ]
}

pub(super) fn unique_requirement_links() -> Vec<PageLink> {
    vec![
        link(
            PageKind::Requirement,
            "req_budget_split",
            "Budget portions shall reconcile",
        ),
        link(
            PageKind::Requirement,
            "req_zero_suppression",
            "Zero claim items shall be suppressed",
        ),
    ]
}

pub(super) fn rule_card(resolver: &LinkResolver) -> RuleCard {
    RuleCard {
        link: link(
            PageKind::Rule,
            "rule_sah_inv_016",
            "Suppress line emission for fully zero claim items",
        ),
        statement: "If a claim item's participant, government, and gap portions are all <= 0 \
                    after markup, no invoice lines shall be emitted for that claim item."
            .to_string(),
        status: RuleStatus::Active,
        severity: RuleSeverity::High,
        evidence: vec![resolver.resolve("src/UseCase.php:153-156")],
    }
}

pub(super) fn field_note(resolver: &LinkResolver) -> FieldNote {
    let body = "Per-portion guard at src/UseCase.php:211-233.\n\
                Confirmed by testCreateGapInvoiceOnly."
        .to_string();
    let refs = resolver.annotate(&body);
    FieldNote {
        message_id: "msg_000001".to_string(),
        role: MessageRole::Assistant,
        created_at: 1_714_780_800_000,
        body,
        refs,
    }
}

pub(super) fn resolution_thread(resolver: &LinkResolver) -> EvidenceThread {
    EvidenceThread {
        thread_id: "thr_resolution_res_split_0".to_string(),
        parent_type: NodeType::Resolution,
        parent_id: "res_split".to_string(),
        status: ThreadStatus::Active,
        messages: vec![field_note(resolver)],
    }
}

pub(super) fn decision(resolver: &LinkResolver) -> DecisionSection {
    DecisionSection {
        link: link(
            PageKind::Resolution,
            "res_split",
            "SaveInvoice per-portion split & $0 suppression extraction",
        ),
        status: ResolutionStatus::Approved,
        position: "Adopt these as 7 rules. Severity high.".to_string(),
        rationale: "Atomicity here = drift detectability.".to_string(),
        context: Some("Codebase scan of UseCase.php identified 7 patterns.".to_string()),
        enforcement: Some("Specification".to_string()),
        confidence: Some(0.97),
        inputs: vec![InputCitation {
            input_type: ResolutionInputType::Technical,
            summary: "Codebase scan — SaveInvoice use case.".to_string(),
            reference: resolver.resolve("src/UseCase.php:59-69"),
        }],
        made_by: Some("Ben Nasraoui".to_string()),
        approved_by: Some("Ben Nasraoui".to_string()),
        approved_at: Some(1_776_470_400_000),
    }
}

pub(super) fn requirement_fixture() -> RequirementPage {
    let resolver = resolver();
    RequirementPage {
        id: PageId::new(RecordKind::Requirement, "req_saveinvoice_split"),
        title: "SaveInvoice shall split each claim item into portions".to_string(),
        status: RequirementStatus::Discovery,
        statement: "Grouping by participant_ref with per-portion positive-amount guards."
            .to_string(),
        description: None,
        fog: None,
        domain_id: Some("dom_invoicing".to_string()),
        domain_has_anchor: true,
        back_link: Some(link(
            PageKind::Requirement,
            "req_sah",
            "Support at Home (SAH)",
        )),
        lineage: vec![
            LineageEntry {
                link: link(PageKind::Requirement, "req_platform", "ExampleOrg platform"),
                is_current: false,
            },
            LineageEntry {
                link: link(PageKind::Requirement, "req_sah", "Support at Home (SAH)"),
                is_current: false,
            },
            LineageEntry {
                link: link(
                    PageKind::Requirement,
                    "req_saveinvoice_split",
                    "SaveInvoice shall split each claim item into portions",
                ),
                is_current: true,
            },
        ],
        decisions: vec![decision(&resolver)],
        produced_rules: vec![rule_card(&resolver)],
        children: vec![link(
            PageKind::Requirement,
            "req_gap_lines",
            "Gap lines shall be suppressed when zero",
        )],
        siblings: vec![],
        supersedes: vec![],
        depends_on: vec![],
        superseded_by: None,
        sources: vec![SourceCitation {
            link: link(PageKind::Source, "source_schads", "SCHADS Award mapping"),
            source_type: SourceType::Document,
            clause: Some("clause 10.3".to_string()),
            reference: Some(resolver.resolve_at("docs/award.md", Some("abc1234"))),
        }],
        gaps: vec![],
        threads: vec![resolution_thread(&resolver)],
    }
}

pub(super) fn gappy_requirement_fixture() -> RequirementPage {
    RequirementPage {
        id: PageId::new(RecordKind::Requirement, "req_stuck"),
        title: "Rostering shall respect awards".to_string(),
        status: RequirementStatus::Resolved,
        statement: "Rostering shall respect awards.".to_string(),
        description: None,
        fog: Some("Which award clauses apply is still unclear.".to_string()),
        domain_id: None,
        domain_has_anchor: false,
        back_link: None,
        lineage: vec![LineageEntry {
            link: link(
                PageKind::Requirement,
                "req_stuck",
                "Rostering shall respect awards",
            ),
            is_current: true,
        }],
        decisions: vec![],
        produced_rules: vec![],
        children: vec![],
        siblings: vec![],
        supersedes: vec![],
        depends_on: vec![],
        superseded_by: None,
        sources: vec![],
        gaps: vec![
            GapNotice {
                kind: GapKind::DanglingReference,
                subject: None,
                related: None,
                detail: "This requirement points to a source that is missing.".to_string(),
            },
            GapNotice {
                kind: GapKind::MissingSourceRefs,
                subject: None,
                related: None,
                detail: "This requirement has no source references.".to_string(),
            },
            GapNotice {
                kind: GapKind::NoResolvingDecision,
                subject: None,
                related: None,
                detail: "This requirement is marked resolved but has no resolving decision."
                    .to_string(),
            },
        ],
        threads: vec![],
    }
}

pub(super) fn resolution_fixture() -> ResolutionPage {
    let resolver = resolver();
    ResolutionPage {
        id: PageId::new(RecordKind::Resolution, "res_split"),
        title: "SaveInvoice per-portion split & $0 suppression extraction".to_string(),
        status: ResolutionStatus::Approved,
        position: "Adopt these as 7 rules. Severity high.".to_string(),
        rationale: "Atomicity here = drift detectability.".to_string(),
        context: Some("Codebase scan of UseCase.php identified 7 patterns.".to_string()),
        enforcement: Some("Specification".to_string()),
        confidence: Some(0.97),
        inputs: vec![InputCitation {
            input_type: ResolutionInputType::Technical,
            summary: "Codebase scan — SaveInvoice use case.".to_string(),
            reference: resolver.resolve("src/UseCase.php:59-69"),
        }],
        made_by: Some("Ben Nasraoui".to_string()),
        approved_by: Some("Ben Nasraoui".to_string()),
        approved_at: Some(1_776_470_400_000),
        review_on: Some("2026-10-01".to_string()),
        superseded_by: None,
        resolves: vec![link(
            PageKind::Requirement,
            "req_saveinvoice_split",
            "SaveInvoice shall split each claim item into portions",
        )],
        spawned: vec![],
        produced_rules: vec![rule_card(&resolver)],
        gaps: vec![],
        threads: vec![resolution_thread(&resolver)],
    }
}

pub(super) fn rule_fixture() -> RulePage {
    let resolver = resolver();
    RulePage {
        id: PageId::new(RecordKind::Rule, "rule_sah_inv_016"),
        title: "Suppress line emission for fully zero claim items".to_string(),
        statement: "No invoice lines shall be emitted for fully zero claim items.".to_string(),
        description: None,
        status: RuleStatus::Active,
        severity: RuleSeverity::High,
        code_scan: Some(CodeScan {
            commit: Some(SCAN_COMMIT.to_string()),
        }),
        implementations: vec![ImplementationBinding {
            symbol: Some("suppress_zero_claim_items".to_string()),
            location: resolver.resolve_at("src/UseCase.php:153", Some(SCAN_COMMIT)),
        }],
        verifications: vec![VerificationSite {
            method: "examples".to_string(),
            symbol: Some("zero_claim_items_emit_no_lines".to_string()),
            location: resolver.resolve_at("tests/UseCaseTest.php:84", Some(SCAN_COMMIT)),
            outside_implementation_module: true,
        }],
        produced_by: vec![link(
            PageKind::Resolution,
            "res_split",
            "SaveInvoice per-portion split & $0 suppression extraction",
        )],
        requirements: vec![link(
            PageKind::Requirement,
            "req_saveinvoice_split",
            "SaveInvoice shall split each claim item into portions",
        )],
        sources: vec![link(
            PageKind::Source,
            "source_schads",
            "SCHADS Award mapping",
        )],
        gaps: vec![],
        threads: vec![],
    }
}

pub(super) fn source_fixture() -> SourcePage {
    let resolver = resolver();
    SourcePage {
        id: PageId::new(RecordKind::Source, "source_schads"),
        title: "SCHADS Award mapping".to_string(),
        source_type: SourceType::Document,
        url: Some("https://example.test/award".to_string()),
        reference: Some(resolver.resolve_at("docs/award.md", Some("abc1234"))),
        commit_pin: Some("abc1234".to_string()),
        effective_date: Some(1_714_780_800_000),
        review_date: None,
        superseded_by: None,
        referenced_requirements: vec![link(
            PageKind::Requirement,
            "req_saveinvoice_split",
            "SaveInvoice shall split each claim item into portions",
        )],
        gaps: vec![],
        threads: vec![],
    }
}

pub(super) fn index_fixture() -> ScopeIndexPage {
    ScopeIndexPage {
        scope: "default".to_string(),
        title: "default documentation".to_string(),
        counts: CorpusCounts {
            sources: 2,
            requirements: 3,
            resolutions: 1,
            rules: 1,
        },
        search_coverage: "Search covers requirements, decisions, rules, and sources.".to_string(),
        search_example: Some("Invoice & participant".to_string()),
        domains: vec![HomepageDomain {
            id: "domain_default".to_string(),
            name: "Invoicing".to_string(),
            description: Some("Invoice behavior".to_string()),
            requirements: 1,
            rules: 1,
        }],
        authored_domain_count: 1,
        unfinished_count: 4,
    }
}

pub(super) fn corpus_fixture() -> WikiCorpus {
    WikiCorpus {
        scope: "default".to_string(),
        index: index_fixture(),
        domains: domain_index_fixture(),
        search: search_fixture(),
        decisions: decisions_fixture(),
        unfinished: unfinished_fixture(),
        requirements: vec![requirement_fixture(), gappy_requirement_fixture()],
        resolutions: vec![resolution_fixture()],
        rules: vec![rule_fixture()],
        sources: vec![source_fixture()],
    }
}

pub(super) fn domain_index_fixture() -> DomainIndexPage {
    DomainIndexPage {
        scope: "default".to_string(),
        title: "Requirements and rules by domain".to_string(),
        authored_group_count: 1,
        groups: vec![
            DomainGroup {
                state: DomainState::Defined {
                    id: "domain_default".to_string(),
                    name: "Invoicing".to_string(),
                    description: Some("Invoice behavior".to_string()),
                },
                requirements: vec![link(
                    PageKind::Requirement,
                    "req_saveinvoice_split",
                    "Invoice & participant",
                )],
                rules: vec![link(
                    PageKind::Rule,
                    "rule_sah_inv_016",
                    "Suppress line emission",
                )],
            },
            DomainGroup {
                state: DomainState::Missing {
                    id: "domain_missing".to_string(),
                },
                requirements: vec![],
                rules: vec![],
            },
            DomainGroup {
                state: DomainState::Unassigned,
                requirements: vec![],
                rules: vec![],
            },
        ],
        all_requirements: vec![SearchEntry {
            link: link(
                PageKind::Requirement,
                "req_saveinvoice_split",
                "Invoice & participant",
            ),
            statement: "Invoice & participant statement".to_string(),
        }],
        all_rules: vec![SearchEntry {
            link: link(PageKind::Rule, "rule_sah_inv_016", "Suppress line emission"),
            statement: "No invoice lines shall be emitted for zero claims".to_string(),
        }],
    }
}
