use provenance_core::protocol::{failure::ErasedFailure, read_failure::ReadFailure};
use provenance_store::operations::catalog::{Definition, ResponseKind};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub fn select_addressed(
    value: &mut Value,
    definition: &Definition,
    path: &BTreeMap<String, String>,
) -> Result<(), ErasedFailure> {
    let Some(entries) = value.as_array_mut() else {
        return Ok(());
    };
    let proposal = path.get("id").filter(|_| {
        matches!(
            definition.backing,
            "list-assertions-v2" | "list-dispositions-v2"
        )
    });
    if definition.response_kind == ResponseKind::Items {
        if let Some(proposal) = proposal {
            entries
                .retain(|entry| entry.get("proposal_id").and_then(Value::as_str) == Some(proposal));
        }
        return Ok(());
    }
    let wanted = path
        .get("fact_id")
        .or_else(|| path.get("message_id"))
        .or_else(|| path.get("id"));
    let Some(wanted) = wanted else {
        return Ok(());
    };
    let found = entries
        .iter()
        .find(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(wanted)
                && proposal.is_none_or(|proposal| {
                    entry.get("proposal_id").and_then(Value::as_str) == Some(proposal)
                })
        })
        .cloned()
        .ok_or_else(|| not_found(definition))?;
    *value = found;
    Ok(())
}

pub fn select_page_member(
    envelope: &mut Value,
    definition: &Definition,
    path: &BTreeMap<String, String>,
) -> Result<(), ErasedFailure> {
    let wanted = path.get("message_id").or_else(|| path.get("discussion_id"));
    let Some(wanted) = wanted else {
        return Ok(());
    };
    let Some(entries) = envelope
        .pointer_mut("/data/entries")
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    let found = entries
        .iter()
        .find(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(wanted)
                || entry
                    .pointer("/discussion/discussion_id")
                    .and_then(Value::as_str)
                    == Some(wanted)
        })
        .cloned()
        .ok_or_else(|| not_found(definition))?;
    envelope["data"] = found;
    Ok(())
}

fn not_found(definition: &Definition) -> ErasedFailure {
    ErasedFailure::declared(definition.name, ReadFailure::ResourceNotFound, 404)
}

pub fn success(mut value: Value, kind: ResponseKind, query: Option<&str>) -> Value {
    let mut meta = Map::new();
    if let Some(object) = value.as_object_mut() {
        for field in [
            "stamp",
            "freshness_error",
            "freshness_cause",
            "limit",
            "has_more",
            "next_cursor",
        ] {
            if let Some(value) = object.remove(field) {
                meta.insert(field.into(), value);
            }
        }
        object.remove("protocol_version");
        object.remove("operation");
        if kind == ResponseKind::Resource && object.get("found") == Some(&json!(true)) {
            value = object.remove("node").unwrap_or(Value::Null);
        }
    }
    if let Some(result) = value
        .as_object_mut()
        .and_then(|object| object.remove("result"))
    {
        value = result;
        if let Some(object) = value.as_object_mut() {
            for field in ["limit", "has_more", "next_cursor"] {
                if let Some(value) = object.remove(field) {
                    meta.insert(field.into(), value);
                }
            }
        }
    }
    let data = if kind == ResponseKind::Items {
        if value.is_array() {
            json!({"items":value})
        } else {
            let fields = match query {
                Some("search") => &["nodes"][..],
                Some("stale") => &["sites"][..],
                Some("resolve-symbol") => &["rules"][..],
                _ => &["items", "entries"][..],
            };
            let items = value
                .as_object_mut()
                .and_then(|object| fields.iter().find_map(|field| object.remove(*field)))
                .unwrap_or(value);
            json!({"items":items})
        }
    } else {
        value
    };
    json!({"data":data,"meta":meta})
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_store::operations::catalog::{ContextKind, HttpMethod};

    fn definition(name: &'static str, kind: ResponseKind) -> Definition {
        Definition {
            name,
            operation_id: name,
            method: HttpMethod::Get,
            path: "/proposals/{id}/assertions/{fact_id}",
            description: "Read one owned assertion for a routing test.",
            mutates: false,
            http_statuses: vec![404],
            parameters: Vec::new(),
            request_schema: None,
            success_schema: json!({}),
            failure_schema: json!({}),
            response_kind: kind,
            backing: "list-assertions-v2",
            context: ContextKind::Scope,
            inject_scope: false,
        }
    }

    #[test]
    fn proposal_member_selection_requires_the_addressed_parent() {
        let mut value = json!([
            {"id":"fact_shared","proposal_id":"proposal_other"},
            {"id":"fact_owned","proposal_id":"proposal_wanted"}
        ]);
        let path = BTreeMap::from([
            ("id".to_owned(), "proposal_wanted".to_owned()),
            ("fact_id".to_owned(), "fact_shared".to_owned()),
        ]);

        let error = select_addressed(
            &mut value,
            &definition("get-proposal-assertion", ResponseKind::Resource),
            &path,
        )
        .unwrap_err();

        assert_eq!(error.status_code(), 404);
        assert_eq!(error.error["kind"], "resource_not_found");
    }

    #[test]
    fn proposal_lists_keep_only_facts_owned_by_the_addressed_parent() {
        let mut value = json!([
            {"id":"fact_other","proposal_id":"proposal_other"},
            {"id":"fact_owned","proposal_id":"proposal_wanted"}
        ]);
        let path = BTreeMap::from([("id".to_owned(), "proposal_wanted".to_owned())]);

        select_addressed(
            &mut value,
            &definition("list-proposal-assertions", ResponseKind::Items),
            &path,
        )
        .unwrap();

        assert_eq!(
            value,
            json!([{"id":"fact_owned","proposal_id":"proposal_wanted"}])
        );
    }
}
