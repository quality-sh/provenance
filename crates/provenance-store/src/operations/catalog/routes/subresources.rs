#![allow(clippy::literal_string_with_formatting_args, clippy::wildcard_imports)]

use super::*;
use crate::operations::catalog as operation;
use crate::operations::catalog::{resource_members as members, resource_pages as pages};

pub(super) fn register(out: &mut Vec<Definition>) {
    history_and_evidence(out);
    review_subresources(out);
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
        vec![
            schema::path("id"),
            schema::query(
                "exclude_terminal",
                serde_json::json!({"type":"boolean","default":false}),
            ),
            limit(),
            cursor(),
        ],
    ));
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

fn review_history_for_kind(
    out: &mut Vec<Definition>,
    plural: &'static str,
    singular: &'static str,
    title: &'static str,
    kind: NodeType,
) {
    let base = leaked(format!("/{plural}/{{id}}/history"));
    out.push(
        backed::<operation::ReviewHistory>(
            leaked(format!("list-{singular}-history")),
            leaked(format!("list{title}History")),
            HttpMethod::Get,
            base,
            "List immutable outcomes for one record.",
            ResponseKind::Items,
            vec![schema::path("id"), limit(), cursor()],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str())
        .items_field("entries")
        .pagination(),
    );
    let entry = leaked(format!("{base}/{{entry_id}}"));
    out.push(
        backed::<operation::ReviewHistoryEntry>(
            leaked(format!("get-{singular}-history-entry")),
            leaked(format!("get{title}HistoryEntry")),
            HttpMethod::Get,
            entry,
            "Read one immutable record outcome.",
            ResponseKind::Result,
            vec![schema::path("id"), schema::path("entry_id")],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str()),
    );
    out.push(
        backed::<operation::ReviewEvidence>(
            leaked(format!("get-{singular}-history-evidence")),
            leaked(format!("get{title}HistoryEvidence")),
            HttpMethod::Get,
            leaked(format!("{entry}/evidence/{{side}}")),
            "Read the before or after evidence for one record outcome.",
            ResponseKind::Result,
            vec![
                schema::path("id"),
                schema::path("entry_id"),
                schema::path("side"),
                schema::query("field", json!({"type":"string"})),
                schema::query("offset", json!({"type":"integer","minimum":0})),
            ],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str())
        .result(),
    );
}

fn review_routes_for_kind(
    out: &mut Vec<Definition>,
    plural: &'static str,
    singular: &'static str,
    title: &'static str,
    kind: NodeType,
) {
    review_history_for_kind(out, plural, singular, title, kind);
    if !matches!(kind, NodeType::Domain | NodeType::Boundary) {
        discussions(out, plural, singular);
    }
}

macro_rules! review_subresources_for_row {
    ($out:ident, [$kind:ident], [requirements], [$review:ident]) => {
        review_routes_for_kind(
            $out,
            "requirements",
            "requirement",
            "Requirement",
            NodeType::$kind,
        );
    };
    (
        $out:ident,
        [$kind:ident],
        [writable {
            mode: $mode:ident,
            plural: $plural:literal,
            singular: $singular:literal,
            singular_id: $singular_id:literal,
            plural_id: $plural_id:literal,
            create: $create:ident,
            update: $update:ident,
            create_defaults: $create_defaults:ident,
            create_aliases: $create_aliases:ident,
            update_defaults: $update_defaults:ident,
            update_aliases: $update_aliases:ident,
            nullable: $nullable:expr,
            target: $target:expr
        }],
        [$review:ident]
    ) => {
        review_routes_for_kind(
            $out,
            $plural,
            $singular,
            $singular_id,
            NodeType::$kind,
        );
    };
    ($out:ident, [$($kind:tt)*], [$($route:tt)*], []) => {};
}

macro_rules! register_review_subresources {
    (
        $out:ident;
        $($group:ident {
            $($variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                meta: $meta:tt,
                node: [$($node:tt)*],
                reader: { open: $reader:ident, closed: [$($closed:tt)*], strategy: $strategy:ident },
                id: $id:ident,
                loader: [$($loader:tt)*],
                graph: [$($graph:tt)*],
                import: [$($import:tt)*],
                catalog: [$($catalog:tt)*],
                route: [$($route:tt)*]
                $(, review: $review:ident)?
            };)*
        })*
    ) => {
        $($(review_subresources_for_row!(
            $out, [$($node)*], [$($route)*], [$($review)?]
        );)*)*
    };
}

fn review_subresources(out: &mut Vec<Definition>) {
    crate::cache::family_table::record_family_rows!(register_review_subresources, out);
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
    proposal_fact::<pages::PageProposalAssertions, members::GetProposalAssertion>(
        out,
        "assertions",
        "assertion",
        "Assertion",
        "listProposalAssertions",
        "getProposalAssertion",
    );
    proposal_fact::<pages::PageProposalDispositions, members::GetProposalDisposition>(
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
    let list = discussion_route::<operation::ReviewDiscussions>(
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
        discussion_route::<operation::WriteDiscussion>(
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
        discussion_route::<operation::ReviewDiscussion>(
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
        discussion_route::<operation::WriteDiscussion>(
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
        discussion_route::<operation::ReviewDiscussionMessages>(
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
        discussion_route::<operation::WriteDiscussion>(
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
        discussion_route::<operation::ReviewDiscussionMessage>(
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
        discussion_route::<operation::ReviewDiscussionMessages>(
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
        discussion_route::<operation::ReviewDiscussionMessage>(
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
