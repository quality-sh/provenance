#![allow(clippy::literal_string_with_formatting_args, clippy::wildcard_imports)]

use super::*;
use crate::operations::catalog as operation;
use crate::operations::catalog::{resource_members as members, resource_pages as pages};

pub(super) fn register(out: &mut Vec<Definition>) {
    history_and_evidence(out);
    for (plural, kind) in [
        ("sources", "source"),
        ("requirements", "requirement"),
        ("resolutions", "resolution"),
        ("rules", "rule"),
        ("topics", "topic"),
        ("questions", "question"),
    ] {
        discussions(out, plural, kind);
    }
    proposal_facts(out);
}

fn history_and_evidence(out: &mut Vec<Definition>) {
    out.push(backed::<operation::ReadDocument>(
        "get-requirement-document",
        "getRequirementDocument",
        HttpMethod::Get,
        "/requirements/{id}/document",
        "Read the assembled document for one Requirement.",
        ResponseKind::Result,
        vec![schema::path("id"), limit(), cursor()],
    ));
    out.push(
        backed::<operation::ReviewHistoryV2>(
            "list-requirement-history",
            "listRequirementHistory",
            HttpMethod::Get,
            "/requirements/{id}/history",
            "List immutable outcomes for one Requirement.",
            ResponseKind::Items,
            vec![schema::path("id"), limit(), cursor()],
        )
        .path_field("id", "requirement_id")
        .items_field("entries")
        .pagination(),
    );
    out.push(
        backed::<operation::ReviewHistoryEntryV2>(
            "get-requirement-history-entry",
            "getRequirementHistoryEntry",
            HttpMethod::Get,
            "/requirements/{id}/history/{entry_id}",
            "Read one immutable Requirement outcome.",
            ResponseKind::Result,
            vec![schema::path("id"), schema::path("entry_id")],
        )
        .path_field("id", "requirement_id"),
    );
    out.push(
        backed::<operation::ReviewEvidenceV2>(
            "get-requirement-history-evidence",
            "getRequirementHistoryEvidence",
            HttpMethod::Get,
            "/requirements/{id}/history/{entry_id}/evidence/{side}",
            "Read the before or after evidence for one Requirement outcome.",
            ResponseKind::Result,
            vec![
                schema::path("id"),
                schema::path("entry_id"),
                schema::path("side"),
                schema::query("field", json!({"type":"string"})),
                schema::query("offset", json!({"type":"integer","minimum":0})),
            ],
        )
        .path_field("id", "requirement_id")
        .result(),
    );
    out.push(
        backed::<operation::Evidence>(
            "get-rule-evidence",
            "getRuleEvidence",
            HttpMethod::Get,
            "/rules/{id}/evidence",
            "Read implementation, verification, and Requirement-change evidence for one Rule.",
            ResponseKind::Result,
            vec![
                schema::path("id"),
                schema::query("base", json!({"type":"string"})),
                schema::query("head", json!({"type":"string"})),
            ],
        )
        .path_field("id", "rule"),
    );
}

fn proposal_facts(out: &mut Vec<Definition>) {
    out.push(
        backed::<operation::CreateAssertion>(
            "create-proposal-assertion",
            "createProposalAssertion",
            HttpMethod::Post,
            "/proposals/{id}/assertions",
            "Add one immutable assertion to a Proposal.",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .path_field("id", "proposal_id")
        .scope("scope_id"),
    );
    out.push(
        backed::<operation::CreateDisposition>(
            "create-proposal-disposition",
            "createProposalDisposition",
            HttpMethod::Post,
            "/proposals/{id}/dispositions",
            "Add one immutable disposition to a Proposal.",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .path_field("id", "proposal_id")
        .scope("scope_id"),
    );
    proposal_fact::<pages::PageProposalAssertionsV2, members::GetProposalAssertionV2>(
        out,
        "assertions",
        "assertion",
        "Assertion",
        "listProposalAssertions",
        "getProposalAssertion",
    );
    proposal_fact::<pages::PageProposalDispositionsV2, members::GetProposalDispositionV2>(
        out,
        "dispositions",
        "disposition",
        "Disposition",
        "listProposalDispositions",
        "getProposalDisposition",
    );
}

fn proposal_fact<L: Operation, G: Operation>(
    out: &mut Vec<Definition>,
    plural: &'static str,
    singular: &'static str,
    _kind: &'static str,
    list_id: &'static str,
    get_id: &'static str,
) {
    let list_path = leaked(format!("/proposals/{{id}}/{plural}"));
    let member_path = leaked(format!("{list_path}/{{fact_id}}"));
    out.push(
        backed::<L>(
            leaked(format!("list-proposal-{plural}")),
            list_id,
            HttpMethod::Get,
            list_path,
            "List immutable facts owned by one Proposal.",
            ResponseKind::Items,
            vec![schema::path("id"), limit(), cursor()],
        )
        .path_field("id", "proposal_id")
        .items_field("items")
        .pagination(),
    );
    out.push(
        backed::<G>(
            leaked(format!("get-proposal-{singular}")),
            get_id,
            HttpMethod::Get,
            member_path,
            "Read one immutable fact owned by one Proposal.",
            ResponseKind::Resource,
            vec![schema::path("id"), schema::path("fact_id")],
        )
        .path_field("id", "proposal_id")
        .result(),
    );
}

fn discussions(out: &mut Vec<Definition>, plural: &'static str, kind: &'static str) {
    let base = leaked(format!("/{plural}/{{id}}/discussions"));
    let list = discussion_route::<operation::ReviewDiscussionsV2>(
        plural,
        kind,
        "list-discussions",
        "ListDiscussions",
        base,
        HttpMethod::Get,
        "List addressed Discussions for the selected parent.",
        ResponseKind::Items,
        vec![schema::path("id"), limit(), cursor()],
    )
    .items_field("entries")
    .pagination();
    out.push(list);
    out.push(
        discussion_route::<operation::WriteDiscussionV2>(
            plural,
            kind,
            "create-discussion",
            "CreateDiscussion",
            base,
            HttpMethod::Post,
            "Start an addressed Discussion for the selected parent.",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .scope("scope_id")
        .adapter(request::DISCUSSION_START)
        .with_request_schema::<operation::StartDiscussionData>()
        .header("Idempotency-Key", "request_id", false)
        .cli_default("actor", CliDefaultValue::String("cli"))
        .with_etag("/version", true),
    );

    let member = leaked(format!("{base}/{{discussion_id}}"));
    out.push(
        discussion_route::<operation::ReviewDiscussionV2>(
            plural,
            kind,
            "get-discussion",
            "GetDiscussion",
            member,
            HttpMethod::Get,
            "Read one addressed Discussion for the selected parent.",
            ResponseKind::Resource,
            vec![schema::path("id"), schema::path("discussion_id")],
        )
        .result()
        .with_etag("/discussion/version", true),
    );
    out.push(
        discussion_route::<operation::WriteDiscussionV2>(
            plural,
            kind,
            "update-discussion",
            "UpdateDiscussion",
            member,
            HttpMethod::Patch,
            "Change the status of one addressed Discussion.",
            ResponseKind::Resource,
            vec![schema::path("id"), schema::path("discussion_id")],
        )
        .scope("scope_id")
        .adapter(request::DISCUSSION_STATUS)
        .with_request_schema::<operation::UpdateDiscussionData>()
        .header("Idempotency-Key", "request_id", false)
        .numeric_header("If-Match", "expected_version")
        .with_etag("/version", true),
    );

    discussion_messages(out, plural, kind, member);
    legacy_messages(out, plural, kind);
}

fn discussion_messages(
    out: &mut Vec<Definition>,
    plural: &'static str,
    kind: &'static str,
    member: &'static str,
) {
    let messages = leaked(format!("{member}/messages"));
    out.push(
        discussion_route::<operation::ReviewDiscussionMessagesV2>(
            plural,
            kind,
            "list-discussion-messages",
            "ListDiscussionMessages",
            messages,
            HttpMethod::Get,
            "List messages in one addressed Discussion.",
            ResponseKind::Items,
            vec![
                schema::path("id"),
                schema::path("discussion_id"),
                limit(),
                cursor(),
            ],
        )
        .selector(SelectorBinding::Discussion {
            parameter: "discussion_id",
            field: "selector",
        })
        .items_field("entries")
        .pagination(),
    );
    out.push(
        discussion_route::<operation::WriteDiscussionV2>(
            plural,
            kind,
            "create-discussion-message",
            "CreateDiscussionMessage",
            messages,
            HttpMethod::Post,
            "Append one message to an addressed Discussion.",
            ResponseKind::Resource,
            vec![schema::path("id"), schema::path("discussion_id")],
        )
        .scope("scope_id")
        .adapter(request::DISCUSSION_REPLY)
        .with_request_schema::<operation::ReplyDiscussionData>()
        .header("Idempotency-Key", "request_id", false)
        .numeric_header("If-Match", "expected_version")
        .cli_default("actor", CliDefaultValue::String("cli"))
        .with_etag("/version", true),
    );
    let message = leaked(format!("{messages}/{{message_id}}"));
    out.push(
        discussion_route::<operation::ReviewDiscussionMessageV2>(
            plural,
            kind,
            "get-discussion-message",
            "GetDiscussionMessage",
            message,
            HttpMethod::Get,
            "Read one message in an addressed Discussion.",
            ResponseKind::Resource,
            vec![
                schema::path("id"),
                schema::path("discussion_id"),
                schema::path("message_id"),
            ],
        )
        .selector(SelectorBinding::Discussion {
            parameter: "discussion_id",
            field: "selector",
        })
        .result(),
    );
}

fn legacy_messages(out: &mut Vec<Definition>, plural: &'static str, kind: &'static str) {
    let list_path = leaked(format!(
        "/{plural}/{{id}}/discussion-containers/{{container_id}}/legacy-messages"
    ));
    out.push(
        discussion_route::<operation::ReviewDiscussionMessagesV2>(
            plural,
            kind,
            "list-legacy-message",
            "ListLegacyMessage",
            list_path,
            HttpMethod::Get,
            "List unassigned historical messages from their parent container.",
            ResponseKind::Items,
            vec![
                schema::path("id"),
                schema::path("container_id"),
                limit(),
                cursor(),
            ],
        )
        .selector(SelectorBinding::Legacy {
            parameter: "container_id",
            field: "selector",
        })
        .items_field("entries")
        .pagination(),
    );
    let member_path = leaked(format!("{list_path}/{{message_id}}"));
    out.push(
        discussion_route::<operation::ReviewDiscussionMessageV2>(
            plural,
            kind,
            "get-legacy-message",
            "GetLegacyMessage",
            member_path,
            HttpMethod::Get,
            "Read one unassigned historical message from its parent container.",
            ResponseKind::Resource,
            vec![
                schema::path("id"),
                schema::path("container_id"),
                schema::path("message_id"),
            ],
        )
        .selector(SelectorBinding::Legacy {
            parameter: "container_id",
            field: "selector",
        })
        .result(),
    );
}

#[allow(clippy::too_many_arguments)]
fn discussion_route<O: Operation>(
    plural: &'static str,
    kind: &'static str,
    name: &'static str,
    operation_suffix: &'static str,
    path: &'static str,
    method: HttpMethod,
    description: &'static str,
    response: ResponseKind,
    parameters: Vec<Parameter>,
) -> Definition {
    backed::<O>(
        leaked(format!("{plural}-{name}")),
        leaked(format!("{kind}{operation_suffix}")),
        method,
        path,
        description,
        response,
        parameters,
    )
    .parent(kind)
}

impl Definition {
    fn with_request_schema<T: schemars::JsonSchema>(mut self) -> Self {
        self.registration.request.schema = Some(schema::request_envelope(
            schema::type_schema::<T>(Contract::Deserialize),
        ));
        self
    }
}

fn limit() -> Parameter {
    schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200}))
}

fn cursor() -> Parameter {
    schema::query("cursor", json!({"type":"string"}))
}

fn leaked(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}
