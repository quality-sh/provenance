#![allow(clippy::too_many_lines, clippy::wildcard_imports)]

use super::*;
use crate::operations::catalog::{BodyBinding, ResponseSelection};

pub(super) fn register(out: &mut Vec<Definition>) {
    resource!(
        out,
        provenance_core::Source,
        "sources",
        "source",
        "Source",
        "Sources",
        "create-source",
        "update-source"
    );
    configure_patch(
        out,
        "update-source",
        &[
            ("url", "url"),
            ("reference", "reference"),
            ("commit_pin", "commit_pin"),
            ("effective_date", "effective_date"),
            ("review_date", "review_date"),
        ],
    );
    let requirements = read::<provenance_core::Requirement>(
        "list-requirements",
        "listRequirements",
        "/requirements",
        "List requirements in the bound scope.",
        "page-requirements-v2",
        ResponseKind::Items,
        list_parameters(true, false),
    )
    .items_field("items")
    .pagination();
    out.push(with_query_results(
        requirements,
        &[("search", ResponseKind::Items)],
        "requirement",
    ));
    let requirement = backed(
        "get-requirement",
        "getRequirement",
        HttpMethod::Get,
        "/requirements/{id}",
        "Read one Requirement with its edit and decision state.",
        "get-requirement-v2",
        ResponseKind::Resource,
        member_parameters(true),
    )
    .with_etag();
    out.push(with_query_results(
        requirement,
        &[
            ("trace", ResponseKind::Result),
            ("neighbors", ResponseKind::Result),
            ("impact", ResponseKind::Result),
        ],
        "requirement",
    ));
    out.push(
        backed(
            "create-requirement",
            "createRequirement",
            HttpMethod::Post,
            "/requirements",
            "Create one Requirement through the guarded review journal.",
            "create-requirement-v2",
            ResponseKind::Resource,
            Vec::new(),
        )
        .header("Idempotency-Key", "request_id", false)
        .with_etag(),
    );
    out.push(
        backed(
            "update-requirement",
            "updateRequirement",
            HttpMethod::Patch,
            "/requirements/{id}",
            "Apply one guarded Requirement text and relationship delta.",
            "update-requirement-v2",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .header("Idempotency-Key", "request_id", false)
        .header("If-Match", "expected_etag", true)
        .null_clears(&[
            ("description", "description"),
            ("fog", "fog"),
            ("domain_id", "domain_id"),
        ])
        .with_etag(),
    );
    resource!(
        out,
        provenance_core::Resolution,
        "resolutions",
        "resolution",
        "Resolution",
        "Resolutions",
        "create-resolution",
        "update-resolution"
    );
    configure_patch(
        out,
        "update-resolution",
        &[
            ("context", "context"),
            ("enforcement", "enforcement"),
            ("confidence", "confidence"),
            ("made_by", "made_by"),
            ("approved_by", "approved_by"),
            ("approved_at", "approved_at"),
            ("review_on", "review_on"),
        ],
    );
    resource!(
        out,
        provenance_core::Rule,
        "rules",
        "rule",
        "Rule",
        "Rules",
        "create-rule",
        "update-rule"
    );
    configure_patch(
        out,
        "update-rule",
        &[
            ("name", "name"),
            ("description", "description"),
            ("source_document", "source_document"),
            ("source_section", "source_section"),
        ],
    );
    resource!(
        out,
        provenance_core::Domain,
        "domains",
        "domain",
        "Domain",
        "Domains",
        "create-domain",
        "update-domain"
    );
    configure_patch(
        out,
        "update-domain",
        &[("description", "description"), ("color", "color")],
    );
    resource!(
        out,
        provenance_core::Boundary,
        "boundaries",
        "boundary",
        "Boundary",
        "Boundaries",
        "create-boundary",
        "update-boundary"
    );
    configure_patch(out, "update-boundary", &[("source_ref", "source_ref")]);
    resource!(
        out,
        provenance_core::Topic,
        "topics",
        "topic",
        "Topic",
        "Topics",
        "create-topic",
        "update-topic"
    );
    resource!(
        out,
        provenance_core::Question,
        "questions",
        "question",
        "Question",
        "Questions",
        "create-question",
        "update-question"
    );
    configure_patch(
        out,
        "update-question",
        &[
            ("resolution_id", "resolution_id"),
            ("contradicts", "contradicts"),
        ],
    );
    resource!(
        out,
        provenance_core::Contribution,
        "contributions",
        "contribution",
        "Contribution",
        "Contributions",
        "create-contribution",
        "upsert-contribution"
    );
    resource!(
        out,
        provenance_core::SynthesisPacket,
        "synthesis-packets",
        "synthesis-packet",
        "SynthesisPacket",
        "SynthesisPackets",
        "create-synthesis-packet",
        "upsert-synthesis-packet"
    );
    resource!(
        out,
        provenance_core::ProposalCard,
        "proposals",
        "proposal",
        "Proposal",
        "Proposals",
        "create-proposal",
        ""
    );
    resource!(
        out,
        provenance_core::VerificationRun,
        "verification-runs",
        "verification-run",
        "VerificationRun",
        "VerificationRuns",
        "",
        ""
    );
    resource!(
        out,
        provenance_core::VerificationBinding,
        "verification-bindings",
        "verification-binding",
        "VerificationBinding",
        "VerificationBindings",
        "",
        ""
    );
    indexes(out);
}

fn configure_patch(
    definitions: &mut [Definition],
    name: &str,
    nullable: &[(&'static str, &'static str)],
) {
    let definition = definitions
        .iter_mut()
        .find(|definition| definition.name == name)
        .expect("registered resource PATCH");
    definition.registration.request.null_clears = nullable
        .iter()
        .map(|(field, clear_name)| NullClearBinding { field, clear_name })
        .collect();
}

fn indexes(out: &mut Vec<Definition>) {
    resource!(
        out,
        provenance_core::Thread,
        "discussion-containers",
        "discussion-container",
        "DiscussionContainer",
        "DiscussionContainers",
        "",
        ""
    );
    resource!(
        out,
        provenance_core::Message,
        "messages",
        "message",
        "Message",
        "Messages",
        "",
        ""
    );
    resource!(
        out,
        provenance_core::AssertionRecord,
        "assertions",
        "assertion",
        "Assertion",
        "Assertions",
        "",
        ""
    );
    resource!(
        out,
        provenance_core::DispositionRecord,
        "dispositions",
        "disposition",
        "Disposition",
        "Dispositions",
        "",
        ""
    );
}
