#![allow(
    clippy::literal_string_with_formatting_args,
    clippy::too_many_lines,
    clippy::wildcard_imports
)]

use super::*;
use crate::operations::catalog::{BodyBinding, PathBinding, ResponseSelection, SelectorBinding};

pub(super) fn register(out: &mut Vec<Definition>) {
    out.push(backed(
        "get-requirement-document",
        "getRequirementDocument",
        HttpMethod::Get,
        "/requirements/{id}/document",
        "Read the assembled document for one Requirement.",
        "read-document",
        ResponseKind::Result,
        vec![
            schema::path("id"),
            schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
            schema::query("cursor", json!({"type":"string"})),
        ],
    ));
    for (name, id, path, description) in [
        (
            "list-requirement-history",
            "listRequirementHistory",
            "/requirements/{id}/history",
            "List immutable outcomes for one Requirement.",
        ),
        (
            "get-requirement-history-entry",
            "getRequirementHistoryEntry",
            "/requirements/{id}/history/{entry_id}",
            "Read one immutable Requirement outcome.",
        ),
        (
            "get-requirement-history-evidence",
            "getRequirementHistoryEvidence",
            "/requirements/{id}/history/{entry_id}/evidence/{side}",
            "Read the before or after evidence for one Requirement outcome.",
        ),
        (
            "get-rule-evidence",
            "getRuleEvidence",
            "/rules/{id}/evidence",
            "Read implementation, verification, and Requirement-change evidence for one Rule.",
        ),
    ] {
        let backing = match name {
            "get-rule-evidence" => "evidence",
            "list-requirement-history" => "review-history-v2",
            "get-requirement-history-entry" => "review-history-entry-v2",
            "get-requirement-history-evidence" => "review-evidence-v2",
            _ => "read-document",
        };
        let mut params: Vec<Parameter> = path
            .split('/')
            .filter_map(|part| part.strip_prefix('{').and_then(|p| p.strip_suffix('}')))
            .map(schema::path)
            .collect();
        if name == "list-requirement-history" {
            params.extend([
                schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
                schema::query("cursor", json!({"type":"string"})),
            ]);
        }
        if name == "get-requirement-history-evidence" {
            params.extend([
                schema::query("field", json!({"type":"string"})),
                schema::query("offset", json!({"type":"integer","minimum":0})),
            ]);
        }
        if name == "get-rule-evidence" {
            params.extend([
                schema::query("base", json!({"type":"string"})),
                schema::query("head", json!({"type":"string"})),
            ]);
        }
        let mut definition = backed(
            name,
            id,
            HttpMethod::Get,
            path,
            description,
            backing,
            if name.starts_with("list-") {
                ResponseKind::Items
            } else {
                ResponseKind::Result
            },
            params,
        );
        if name == "get-rule-evidence" {
            definition = definition.path_field("id", "rule");
        } else if name.starts_with("list-requirement-history")
            || name.starts_with("get-requirement-history")
        {
            definition = definition.path_field("id", "requirement_id");
        }
        if name == "list-requirement-history" {
            definition = definition.items_field("entries").pagination();
        }
        out.push(definition);
    }
    for parent in [
        "sources",
        "requirements",
        "resolutions",
        "rules",
        "topics",
        "questions",
    ] {
        discussions(out, parent);
    }
    out.push(
        backed(
            "create-proposal-assertion",
            "createProposalAssertion",
            HttpMethod::Post,
            "/proposals/{id}/assertions",
            "Add one immutable assertion to a Proposal.",
            "create-assertion",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .path_field("id", "proposal_id"),
    );
    out.push(
        backed(
            "create-proposal-disposition",
            "createProposalDisposition",
            HttpMethod::Post,
            "/proposals/{id}/dispositions",
            "Add one immutable disposition to a Proposal.",
            "create-disposition",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .path_field("id", "proposal_id"),
    );
    for (kind, plural, backing) in [
        ("Assertion", "assertions", "list-assertions-v2"),
        ("Disposition", "dispositions", "list-dispositions-v2"),
    ] {
        let list_path: &'static str =
            Box::leak(format!("/proposals/{{id}}/{plural}").into_boxed_str());
        let member_path: &'static str =
            Box::leak(format!("{list_path}/{{fact_id}}").into_boxed_str());
        let list_name: &'static str = Box::leak(format!("list-proposal-{plural}").into_boxed_str());
        let get_name: &'static str =
            Box::leak(format!("get-proposal-{}", kind.to_ascii_lowercase()).into_boxed_str());
        let list_id: &'static str = Box::leak(format!("listProposal{kind}s").into_boxed_str());
        let get_id: &'static str = Box::leak(format!("getProposal{kind}").into_boxed_str());
        out.push(
            backed(
                list_name,
                list_id,
                HttpMethod::Get,
                list_path,
                "List immutable facts owned by one Proposal.",
                backing,
                ResponseKind::Items,
                vec![schema::path("id")],
            )
            .body(BodyBinding::Null)
            .response_selection(ResponseSelection::ArrayItems {
                owner_parameter: "id",
                owner_field: "proposal_id",
            }),
        );
        out.push(
            backed(
                get_name,
                get_id,
                HttpMethod::Get,
                member_path,
                "Read one immutable fact owned by one Proposal.",
                backing,
                ResponseKind::Resource,
                vec![schema::path("id"), schema::path("fact_id")],
            )
            .body(BodyBinding::Null)
            .response_selection(ResponseSelection::ArrayMember {
                id_parameter: "fact_id",
                owner_parameter: Some(("id", "proposal_id")),
            }),
        );
    }
}

fn discussions(out: &mut Vec<Definition>, parent: &'static str) {
    let base: &'static str = Box::leak(format!("/{parent}/{{id}}/discussions").into_boxed_str());
    for (suffix, method, name, id, backing, kind, description) in [
        (
            "",
            HttpMethod::Get,
            "list-discussions",
            "listDiscussions",
            "review-discussions-v2",
            ResponseKind::Items,
            "List addressed Discussions for the selected parent.",
        ),
        (
            "",
            HttpMethod::Post,
            "create-discussion",
            "createDiscussion",
            "write-discussion-v2",
            ResponseKind::Resource,
            "Start an addressed Discussion for the selected parent.",
        ),
        (
            "/{discussion_id}",
            HttpMethod::Get,
            "get-discussion",
            "getDiscussion",
            "review-discussions-v2",
            ResponseKind::Resource,
            "Read one addressed Discussion for the selected parent.",
        ),
        (
            "/{discussion_id}",
            HttpMethod::Patch,
            "update-discussion",
            "updateDiscussion",
            "write-discussion-v2",
            ResponseKind::Resource,
            "Change the status of one addressed Discussion.",
        ),
        (
            "/{discussion_id}/messages",
            HttpMethod::Get,
            "list-discussion-messages",
            "listDiscussionMessages",
            "review-discussion-messages-v2",
            ResponseKind::Items,
            "List messages in one addressed Discussion.",
        ),
        (
            "/{discussion_id}/messages",
            HttpMethod::Post,
            "create-discussion-message",
            "createDiscussionMessage",
            "write-discussion-v2",
            ResponseKind::Resource,
            "Append one message to an addressed Discussion.",
        ),
        (
            "/{discussion_id}/messages/{message_id}",
            HttpMethod::Get,
            "get-discussion-message",
            "getDiscussionMessage",
            "review-discussion-messages-v2",
            ResponseKind::Resource,
            "Read one message in an addressed Discussion.",
        ),
    ] {
        let path: &'static str = Box::leak(format!("{base}{suffix}").into_boxed_str());
        let tool: &'static str = Box::leak(format!("{parent}-{name}").into_boxed_str());
        let mut id_chars = id.chars();
        let action = id_chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(id_chars).collect()
        });
        let operation: &'static str =
            Box::leak(format!("{}{action}", parent.trim_end_matches('s')).into_boxed_str());
        let params = path
            .split('/')
            .filter_map(|p| p.strip_prefix('{').and_then(|p| p.strip_suffix('}')))
            .map(schema::path)
            .collect();
        let mut params: Vec<Parameter> = params;
        if matches!(name, "list-discussions" | "list-discussion-messages") {
            params.extend([
                schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
                schema::query("cursor", json!({"type":"string"})),
            ]);
        }
        let member_message = name == "get-discussion-message";
        let backing = if member_message {
            "review-discussion-message-v2"
        } else {
            backing
        };
        let mut definition = backed(
            tool,
            operation,
            method,
            path,
            description,
            backing,
            kind,
            params,
        );
        definition.registration.request.path.clear();
        definition = definition.parent(parent.trim_end_matches('s'));
        if matches!(name, "list-discussion-messages" | "get-discussion-message") {
            definition = definition.selector(SelectorBinding::Discussion {
                parameter: "discussion_id",
                field: "selector",
            });
        }
        if member_message {
            definition.registration.request.path.push(PathBinding {
                parameter: "message_id",
                field: "message_id",
            });
        }
        if matches!(name, "list-discussions" | "list-discussion-messages") {
            definition = definition.items_field("entries").pagination();
        }
        if name == "get-discussion" {
            definition = definition.response_selection(ResponseSelection::PageMember {
                id_parameter: "discussion_id",
                id_pointer: "/discussion/discussion_id",
            });
        }
        if name == "get-discussion" {
            definition.success_schema = schema::response_envelope(
                schema::type_schema::<provenance_core::threads::DiscussionGroup>(
                    Contract::Serialize,
                ),
                ResponseKind::Resource,
            );
        } else if name == "get-discussion-message" {
            definition.success_schema = schema::response_envelope(
                schema::type_schema::<provenance_core::Message>(Contract::Serialize),
                ResponseKind::Resource,
            );
        }
        if name == "create-discussion" {
            definition.request_schema = Some(schema::request_envelope(schema::type_schema::<
                super::super::StartDiscussionData,
            >(
                Contract::Deserialize
            )));
            definition = definition
                .body(BodyBinding::DiscussionStart)
                .header("Idempotency-Key", "request_id", false)
                .with_etag();
        } else if name == "create-discussion-message" {
            definition.request_schema = Some(schema::request_envelope(schema::type_schema::<
                super::super::ReplyDiscussionData,
            >(
                Contract::Deserialize
            )));
            definition = definition
                .body(BodyBinding::DiscussionReply)
                .header("Idempotency-Key", "request_id", false)
                .numeric_header("If-Match", "expected_version")
                .with_etag();
        } else if name == "update-discussion" {
            definition.request_schema = Some(schema::request_envelope(schema::type_schema::<
                super::super::UpdateDiscussionData,
            >(
                Contract::Deserialize
            )));
            definition = definition
                .body(BodyBinding::DiscussionStatus)
                .header("Idempotency-Key", "request_id", false)
                .numeric_header("If-Match", "expected_version")
                .with_etag();
        } else if matches!(name, "get-discussion" | "get-discussion-message") {
            definition = definition.with_etag();
        }
        out.push(definition);
    }
    for suffix in ["", "/{message_id}"] {
        let path: &'static str = Box::leak(
            format!(
                "/{parent}/{{id}}/discussion-containers/{{container_id}}/legacy-messages{suffix}"
            )
            .into_boxed_str(),
        );
        let one = !suffix.is_empty();
        let name: &'static str = Box::leak(
            format!(
                "{parent}-{}legacy-message",
                if one { "get-" } else { "list-" }
            )
            .into_boxed_str(),
        );
        let id: &'static str = Box::leak(
            format!(
                "{}{}LegacyMessage",
                parent.trim_end_matches('s'),
                if one { "Get" } else { "List" }
            )
            .into_boxed_str(),
        );
        let mut params: Vec<Parameter> = path
            .split('/')
            .filter_map(|p| p.strip_prefix('{').and_then(|p| p.strip_suffix('}')))
            .map(schema::path)
            .collect();
        if !one {
            params.extend([
                schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
                schema::query("cursor", json!({"type":"string"})),
            ]);
        }
        let backing = if one {
            "review-discussion-message-v2"
        } else {
            "review-discussion-messages-v2"
        };
        let mut definition = backed(
            name,
            id,
            HttpMethod::Get,
            path,
            if one {
                "Read one unassigned historical message from its parent container."
            } else {
                "List unassigned historical messages from their parent container."
            },
            backing,
            if one {
                ResponseKind::Resource
            } else {
                ResponseKind::Items
            },
            params,
        );
        definition.registration.request.path.clear();
        definition =
            definition
                .parent(parent.trim_end_matches('s'))
                .selector(SelectorBinding::Legacy {
                    parameter: "container_id",
                    field: "selector",
                });
        if one {
            definition.registration.request.path.push(PathBinding {
                parameter: "message_id",
                field: "message_id",
            });
            definition.success_schema = schema::response_envelope(
                schema::type_schema::<provenance_core::Message>(Contract::Serialize),
                ResponseKind::Resource,
            );
        } else {
            definition = definition.items_field("entries").pagination();
        }
        out.push(definition);
    }
}
