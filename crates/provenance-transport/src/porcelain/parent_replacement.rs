//! Selected parent replacement through two guarded Requirement updates.

use super::{Action, TargetRoute};
use axum::http::{HeaderMap, HeaderValue, Method};
use provenance_core::protocol::failure::ErasedFailure;
use provenance_core::NodeType;
use provenance_macros::rule;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The parent that an update replaces, and the parent that takes its place.
#[derive(Clone, Copy, Debug)]
pub struct ParentReplacement<'a> {
    pub replace_parent: &'a str,
    pub with_parent: &'a str,
}

/// The result of a replacement.
#[derive(Debug)]
pub struct Replaced {
    /// The Requirement after the last write, or as read when nothing changed.
    pub record: Value,
    /// False when the Requirement had the new parent and not the old one, so the
    /// replacement made no write.
    pub changed: bool,
}

/// A replacement that does not start or does not finish.
#[derive(Debug)]
pub enum ReplacementError {
    /// The fields of the call cannot make a replacement. The text names the fault.
    Invalid(String),
    /// The text tells why nothing changed.
    Unchanged(String),
    /// A read or a write of the Requirement failed.
    Failure(Box<ErasedFailure>),
}

impl std::fmt::Display for ReplacementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) | Self::Unchanged(message) => formatter.write_str(message),
            Self::Failure(failure) => write!(formatter, "{failure:?}"),
        }
    }
}

impl std::error::Error for ReplacementError {}

impl crate::StatementHost {
    /// Adds the new parent before it removes the selected old parent.
    ///
    /// The current parents select the steps: a Requirement that already has the new
    /// parent and not the old one is a finished replacement if the old parent exists,
    /// and a Requirement that has neither parent is refused before any write. A
    /// finished replacement makes no write, so it does not check `if_match` or
    /// `declared_by`.
    #[rule("rule_move_replaces_only_selected_parent")]
    #[rule("rule_move_preserves_identity_and_descendants")]
    #[rule("rule_supersede_swaps_selected_child_parent")]
    pub async fn replace_parent(
        &self,
        route: &TargetRoute,
        mut data: Value,
        headers: HeaderMap,
        parents: ParentReplacement<'_>,
    ) -> Result<Replaced, ReplacementError> {
        let (old, new) = (parents.replace_parent, parents.with_parent);
        let invalid = |message: &str| ReplacementError::Invalid(message.to_owned());
        if route.action != Action::Update || route.kind != NodeType::Requirement {
            return Err(invalid("parent replacement requires a Requirement update"));
        }
        for id in [old, new] {
            provenance_core::StableId::new(id)
                .map_err(|error| ReplacementError::Invalid(error.to_string()))?;
        }
        if old == new {
            return Err(invalid("the old and new parents must differ"));
        }
        if !data.as_object().is_some_and(|fields| {
            fields
                .keys()
                .all(|key| matches!(key.as_str(), "actor" | "declared_by"))
        }) {
            return Err(invalid(
                "parent replacement accepts only actor, declared_by, and the parent fields",
            ));
        }
        let path = route.path.as_str();
        let read = self.read_requirement(path).await?;
        let parents = read["data"]["refines"].as_array();
        let has =
            |id: &str| parents.is_some_and(|parents| parents.iter().any(|parent| parent == id));
        let (has_old, has_new) = (has(old), has(new));
        if !has_old {
            if !has_new {
                return Err(ReplacementError::Unchanged(format!(
                    "{old} is not a parent of this Requirement; nothing changed"
                )));
            }
            let (collection, _) = path.rsplit_once('/').expect("Requirement path has an ID");
            match self.read_requirement(&format!("{collection}/{old}")).await {
                Ok(_) => {}
                Err(ReplacementError::Failure(failure)) if failure.status_code() == 404 => {
                    return Err(ReplacementError::Unchanged(format!(
                        "Requirement {old} does not exist; nothing changed"
                    )));
                }
                Err(error) => return Err(error),
            }
            return Ok(Replaced {
                record: read,
                changed: false,
            });
        }
        let mut guard = headers;
        if !guard.contains_key("if-match") {
            guard.insert("if-match", etag(&read)?);
        }
        if !has_new {
            data["relationships"] = json!({"refines":{"add":[new]}});
            let added = self.patch_requirement(path, data.clone(), guard).await?;
            // Use the first write's guard so a concurrent edit refuses the removal.
            guard = HeaderMap::new();
            guard.insert("if-match", etag(&added)?);
        }
        data["relationships"] = json!({"refines":{"remove":[old]}});
        Ok(Replaced {
            record: self.patch_requirement(path, data, guard).await?,
            changed: true,
        })
    }

    async fn read_requirement(&self, path: &str) -> Result<Value, ReplacementError> {
        self.invoke_resource(
            Method::GET,
            path,
            json!({}),
            BTreeMap::new(),
            HeaderMap::new(),
        )
        .await
        .map_err(|failure| ReplacementError::Failure(Box::new(failure)))
    }

    async fn patch_requirement(
        &self,
        path: &str,
        data: Value,
        guard: HeaderMap,
    ) -> Result<Value, ReplacementError> {
        self.invoke_resource(Method::PATCH, path, data, BTreeMap::new(), guard)
            .await
            .map_err(|failure| ReplacementError::Failure(Box::new(failure)))
    }
}

fn etag(record: &Value) -> Result<HeaderValue, ReplacementError> {
    record["data"]["edit"]["etag"]
        .as_str()
        .and_then(|etag| etag.parse().ok())
        .ok_or_else(|| ReplacementError::Invalid("Requirement read has no ETag".into()))
}
