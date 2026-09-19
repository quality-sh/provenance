#![allow(clippy::too_many_lines, clippy::wildcard_imports)]

use super::*;
use crate::operations::catalog::{resource_lists as lists, resource_pages as pages};

const NONE: &[CliDefault] = &[];
const NO_ALIASES: &[ArgumentAlias] = &[];
const CREATE_REQUIREMENT_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "actor",
        value: CliDefaultValue::String("cli"),
    },
    CliDefault {
        field: "status",
        value: CliDefaultValue::String("active"),
    },
    CliDefault {
        field: "depends_on",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "supersedes",
        value: CliDefaultValue::EmptyArray,
    },
];
const UPDATE_REQUIREMENT_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "actor",
        value: CliDefaultValue::String("cli"),
    },
    CliDefault {
        field: "clear_fields",
        value: CliDefaultValue::EmptyArray,
    },
];
const CREATE_SOURCE_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "source_type",
        value: CliDefaultValue::String("policy"),
    },
    CliDefault {
        field: "supersedes",
        value: CliDefaultValue::EmptyArray,
    },
];
const CREATE_RULE_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "requirement_ids",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "resolution_ids",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "status",
        value: CliDefaultValue::String("active"),
    },
    CliDefault {
        field: "severity",
        value: CliDefaultValue::String("high"),
    },
];
const CREATE_RESOLUTION_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "requirement_ids",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "supersedes",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "inputs",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "status",
        value: CliDefaultValue::String("proposed"),
    },
];
const OPEN_LINK_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "status",
        value: CliDefaultValue::String("open"),
    },
    CliDefault {
        field: "links",
        value: CliDefaultValue::EmptyArray,
    },
];
const RULE_ALIASES: &[ArgumentAlias] = &[
    ArgumentAlias {
        argument: "requirement_id",
        field: "requirement_ids",
        wrap_array: true,
    },
    ArgumentAlias {
        argument: "resolution_id",
        field: "resolution_ids",
        wrap_array: true,
    },
];
const RESOLUTION_ALIASES: &[ArgumentAlias] = &[ArgumentAlias {
    argument: "requirement_id",
    field: "requirement_ids",
    wrap_array: true,
}];
const QUESTION_ALIASES: &[ArgumentAlias] = &[ArgumentAlias {
    argument: "method",
    field: "resolution_method",
    wrap_array: false,
}];

pub(super) fn register(out: &mut Vec<Definition>) {
    resource!(
        out,
        searchable,
        provenance_core::Source,
        pages::PageSourcesV2,
        lists::ListSourcesV2,
        "sources",
        "source",
        "Source",
        "Sources",
        super::super::CreateSource,
        super::super::UpdateSource,
        CREATE_SOURCE_DEFAULTS,
        NO_ALIASES,
        NONE,
        NO_ALIASES,
        &[
            ("url", "url"),
            ("reference", "reference"),
            ("commit_pin", "commit_pin"),
            ("effective_date", "effective_date"),
            ("review_date", "review_date")
        ]
    );
    requirements(out);
    resource!(
        out,
        searchable,
        provenance_core::Resolution,
        pages::PageResolutionsV2,
        lists::ListResolutionsV2,
        "resolutions",
        "resolution",
        "Resolution",
        "Resolutions",
        super::super::CreateResolution,
        super::super::UpdateResolution,
        CREATE_RESOLUTION_DEFAULTS,
        RESOLUTION_ALIASES,
        NONE,
        NO_ALIASES,
        &[
            ("context", "context"),
            ("enforcement", "enforcement"),
            ("confidence", "confidence"),
            ("made_by", "made_by"),
            ("approved_by", "approved_by"),
            ("approved_at", "approved_at"),
            ("review_on", "review_on")
        ]
    );
    resource!(
        out,
        rules,
        provenance_core::Rule,
        pages::PageRulesV2,
        lists::ListRulesV2,
        "rules",
        "rule",
        "Rule",
        "Rules",
        super::super::CreateRule,
        super::super::UpdateRule,
        CREATE_RULE_DEFAULTS,
        RULE_ALIASES,
        NONE,
        NO_ALIASES,
        &[
            ("name", "name"),
            ("description", "description"),
            ("source_document", "source_document"),
            ("source_section", "source_section")
        ]
    );
    resource!(
        out,
        searchable,
        provenance_core::Domain,
        pages::PageDomainsV2,
        lists::ListDomainsV2,
        "domains",
        "domain",
        "Domain",
        "Domains",
        super::super::CreateDomain,
        super::super::UpdateDomain,
        NONE,
        NO_ALIASES,
        NONE,
        NO_ALIASES,
        &[("description", "description"), ("color", "color")]
    );
    resource!(
        out,
        searchable,
        provenance_core::Boundary,
        pages::PageBoundariesV2,
        lists::ListBoundariesV2,
        "boundaries",
        "boundary",
        "Boundary",
        "Boundaries",
        super::super::CreateBoundary,
        super::super::UpdateBoundary,
        NONE,
        NO_ALIASES,
        NONE,
        NO_ALIASES,
        &[("source_ref", "source_ref")]
    );
    resource!(
        out,
        searchable,
        provenance_core::Topic,
        pages::PageTopicsV2,
        lists::ListTopicsV2,
        "topics",
        "topic",
        "Topic",
        "Topics",
        super::super::CreateTopic,
        super::super::UpdateTopic,
        OPEN_LINK_DEFAULTS,
        NO_ALIASES,
        NONE,
        NO_ALIASES,
        &[]
    );
    resource!(
        out,
        searchable,
        provenance_core::Question,
        pages::PageQuestionsV2,
        lists::ListQuestionsV2,
        "questions",
        "question",
        "Question",
        "Questions",
        super::super::CreateQuestion,
        super::super::UpdateQuestion,
        OPEN_LINK_DEFAULTS,
        QUESTION_ALIASES,
        NONE,
        QUESTION_ALIASES,
        &[
            ("resolution_id", "resolution_id"),
            ("contradicts", "contradicts")
        ]
    );
    resource!(
        out,
        plain,
        provenance_core::Contribution,
        lists::ListContributionsV2,
        lists::ListContributionsV2,
        "contributions",
        "contribution",
        "Contribution",
        "Contributions",
        super::super::CreateContribution,
        super::super::UpsertContribution,
        NONE,
        NO_ALIASES,
        NONE,
        NO_ALIASES,
        &[]
    );
    resource!(
        out,
        plain,
        provenance_core::SynthesisPacket,
        lists::ListSynthesisPacketsV2,
        lists::ListSynthesisPacketsV2,
        "synthesis-packets",
        "synthesis-packet",
        "SynthesisPacket",
        "SynthesisPackets",
        super::super::CreateSynthesisPacket,
        super::super::UpsertSynthesisPacket,
        NONE,
        NO_ALIASES,
        NONE,
        NO_ALIASES,
        &[]
    );
    resource!(
        out,
        plain,
        provenance_core::ProposalCard,
        super::super::ListProposals,
        super::super::ListProposals,
        "proposals",
        "proposal",
        "Proposal",
        "Proposals"
    );
    out.push(
        backed::<super::super::CreateProposal>(
            "create-proposal",
            "createProposal",
            HttpMethod::Post,
            "/proposals",
            "Create one proposal in the bound scope.",
            ResponseKind::Resource,
            Vec::new(),
        )
        .scope("scope_id"),
    );
    resource!(
        out,
        verification,
        provenance_core::VerificationRun,
        super::super::VerificationRuns,
        lists::ListVerificationRunsV2,
        "verification-runs",
        "verification-run",
        "VerificationRun",
        "VerificationRuns"
    );
    resource!(
        out,
        verification,
        provenance_core::VerificationBinding,
        super::super::VerificationBindings,
        lists::ListVerificationBindingsV2,
        "verification-bindings",
        "verification-binding",
        "VerificationBinding",
        "VerificationBindings"
    );
    indexes(out);
}

fn requirements(out: &mut Vec<Definition>) {
    let list = read::<provenance_core::Requirement, pages::PageRequirementsV2>(
        "list-requirements",
        "listRequirements",
        "/requirements",
        "List requirements in the bound scope.",
        ResponseKind::Items,
        list_parameters(true, false),
    )
    .items_field("items")
    .pagination();
    let queries = searchable_queries(&list, "requirement");
    out.push(with_query_results(list, queries));
    let member = backed::<super::super::GetRequirementV2>(
        "get-requirement",
        "getRequirement",
        HttpMethod::Get,
        "/requirements/{id}",
        "Read one Requirement with its edit and decision state.",
        ResponseKind::Resource,
        member_parameters(true),
    )
    .with_etag("/edit/etag", false);
    let queries = member_queries(&member, "requirement");
    out.push(with_query_results(member, queries));
    out.push(
        backed::<super::super::CreateRequirementV2>(
            "create-requirement",
            "createRequirement",
            HttpMethod::Post,
            "/requirements",
            "Create one Requirement through the guarded review journal.",
            ResponseKind::Resource,
            Vec::new(),
        )
        .header("Idempotency-Key", "request_id", false)
        .cli_defaults(CREATE_REQUIREMENT_DEFAULTS)
        .with_etag("/edit/etag", false),
    );
    out.push(
        backed::<super::super::UpdateRequirementV2>(
            "update-requirement",
            "updateRequirement",
            HttpMethod::Patch,
            "/requirements/{id}",
            "Apply one guarded Requirement text and relationship delta.",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .header("Idempotency-Key", "request_id", false)
        .header("If-Match", "expected_etag", true)
        .cli_defaults(UPDATE_REQUIREMENT_DEFAULTS)
        .null_clears(&[
            ("description", "description"),
            ("fog", "fog"),
            ("domain_id", "domain_id"),
        ])
        .with_etag("/edit/etag", false),
    );
}

fn indexes(out: &mut Vec<Definition>) {
    resource!(
        out,
        plain,
        provenance_core::Thread,
        lists::ListDiscussionContainersV2,
        lists::ListDiscussionContainersV2,
        "discussion-containers",
        "discussion-container",
        "DiscussionContainer",
        "DiscussionContainers"
    );
    resource!(
        out,
        plain,
        provenance_core::Message,
        super::super::ListMessages,
        super::super::ListMessages,
        "messages",
        "message",
        "Message",
        "Messages"
    );
    resource!(
        out,
        plain,
        provenance_core::AssertionRecord,
        super::super::ListAssertions,
        super::super::ListAssertions,
        "assertions",
        "assertion",
        "Assertion",
        "Assertions"
    );
    resource!(
        out,
        plain,
        provenance_core::DispositionRecord,
        super::super::ListDispositions,
        super::super::ListDispositions,
        "dispositions",
        "disposition",
        "Disposition",
        "Dispositions"
    );
}
