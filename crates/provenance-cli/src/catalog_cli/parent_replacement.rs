//! Selected parent replacement through two guarded Requirement updates.
use axum::http::{HeaderMap, Method};
use provenance_macros::rule;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Adds the new parent before it removes the selected old parent.
#[rule("rule_move_replaces_only_selected_parent")]
#[rule("rule_move_preserves_identity_and_descendants")]
#[rule("rule_supersede_swaps_selected_child_parent")]
pub(super) async fn replace(
    host: &StatementHost,
    path: &str,
    mut data: Value,
    headers: HeaderMap,
    old: &str,
    new: &str,
) -> anyhow::Result<Value> {
    provenance_core::StableId::new(old)?;
    provenance_core::StableId::new(new)?;
    anyhow::ensure!(old != new, "the old and new parents must differ");
    anyhow::ensure!(
        data.as_object()
            .is_some_and(|fields| fields.keys().all(|key| key == "actor")),
        "parent replacement accepts only --actor and the parent options"
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
        .map_err(anyhow::Error::new)?;
    let mut first_headers = headers;
    if !first_headers.contains_key("if-match") {
        first_headers.insert(
            "if-match",
            read["data"]["edit"]["etag"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Requirement read has no ETag"))?
                .parse()?,
        );
    }
    data["relationships"] = json!({"refines":{"add":[new]}});
    let added = host
        .invoke_resource(
            Method::PATCH,
            path,
            data.clone(),
            BTreeMap::new(),
            first_headers,
        )
        .await
        .map_err(anyhow::Error::new)?;
    // Use the first write's guard so a concurrent edit refuses the removal.
    let mut second_headers = HeaderMap::new();
    second_headers.insert(
        "if-match",
        added["data"]["edit"]["etag"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Requirement read has no ETag"))?
            .parse()?,
    );
    data["relationships"] = json!({"refines":{"remove":[old]}});
    host.invoke_resource(Method::PATCH, path, data, BTreeMap::new(), second_headers)
        .await
        .map_err(anyhow::Error::new)
}
