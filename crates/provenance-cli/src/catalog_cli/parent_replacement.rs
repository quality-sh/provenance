//! Selected parent replacement through two guarded Requirement updates.
use axum::http::{HeaderMap, Method};
use provenance_core::protocol::failure::ErasedFailure;
use provenance_macros::rule;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Adds the new parent before it removes the selected old parent.
///
/// The current parents select the steps: a Requirement that already has the
/// new parent and not the old one is a finished replacement, and a
/// Requirement that has neither parent is refused before any write.
#[rule("rule_move_replaces_only_selected_parent")]
#[rule("rule_move_preserves_identity_and_descendants")]
#[rule("rule_supersede_swaps_selected_child_parent")]
pub(super) async fn replace(
    host: &StatementHost,
    path: &str,
    mut data: Value,
    headers: HeaderMap,
    (old, new): (&str, &str),
    guidance: impl Fn(&ErasedFailure) -> anyhow::Error,
) -> anyhow::Result<Value> {
    provenance_core::StableId::new(old)?;
    provenance_core::StableId::new(new)?;
    anyhow::ensure!(old != new, "the old and new parents must differ");
    anyhow::ensure!(
        data.as_object().is_some_and(|fields| fields
            .keys()
            .all(|key| matches!(key.as_str(), "actor" | "declared_by"))),
        "parent replacement accepts only --actor, --declared-by, and the parent options"
    );
    let read = host
        .invoke_resource(
            Method::GET,
            path,
            json!({}),
            BTreeMap::new(),
            HeaderMap::new(),
        )
        .await
        .map_err(|failure| guidance(&failure))?;
    let parents = read["data"]["refines"].as_array();
    let has = |id: &str| parents.is_some_and(|parents| parents.iter().any(|parent| parent == id));
    let (has_old, has_new) = (has(old), has(new));
    if !has_old {
        anyhow::ensure!(
            has_new,
            "{old} is not a parent of this Requirement; nothing changed"
        );
        return Ok(read);
    }
    let mut guard = headers;
    if !guard.contains_key("if-match") {
        guard.insert("if-match", etag(&read)?);
    }
    if !has_new {
        data["relationships"] = json!({"refines":{"add":[new]}});
        let added = host
            .invoke_resource(Method::PATCH, path, data.clone(), BTreeMap::new(), guard)
            .await
            .map_err(|failure| guidance(&failure))?;
        // Use the first write's guard so a concurrent edit refuses the removal.
        guard = HeaderMap::new();
        guard.insert("if-match", etag(&added)?);
    }
    data["relationships"] = json!({"refines":{"remove":[old]}});
    host.invoke_resource(Method::PATCH, path, data, BTreeMap::new(), guard)
        .await
        .map_err(|failure| guidance(&failure))
}

fn etag(record: &Value) -> anyhow::Result<axum::http::HeaderValue> {
    Ok(record["data"]["edit"]["etag"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Requirement read has no ETag"))?
        .parse()?)
}
