use crate::{cache::review_families, canonical_digest};
use provenance_core::NodeType;
use serde::Serialize;

pub(super) fn changed_fields<T: Serialize>(
    kind: NodeType,
    before: &T,
    after: &T,
) -> anyhow::Result<Vec<String>> {
    let before = provenance_core::model::record_stamps::content_value(before)?;
    let after = provenance_core::model::record_stamps::content_value(after)?;
    let mut names = before
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("record does not serialize to an object"))?
        .keys()
        .chain(
            after
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("record does not serialize to an object"))?
                .keys(),
        )
        .filter(|name| name.as_str() != "schema_version")
        .cloned()
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names.retain(|name| before.get(name) != after.get(name));
    let facts = review_families::by_kind(kind);
    anyhow::ensure!(
        names.iter().all(|name| {
            facts.content_fields.contains(&name.as_str())
                || facts.lifecycle_fields.contains(&name.as_str())
        }),
        "record contains a field with no review classification"
    );
    Ok(names)
}

/// Classifies whether changed fields create a new review revision.
#[provenance_macros::rule("rule_content_change_requires_new_review")]
pub(super) fn changes_revision(kind: NodeType, fields: &[String]) -> bool {
    let content = review_families::by_kind(kind).content_fields;
    fields.iter().any(|field| content.contains(&field.as_str()))
}

/// Digests the review-content fields of one record. Lifecycle fields and
/// record stamps are left out, so a lifecycle-only save keeps the digest a
/// submission bound, and a decision on the reviewed content stays possible
/// after one.
pub(super) fn content_digest<T: Serialize>(kind: NodeType, record: &T) -> anyhow::Result<String> {
    let value = provenance_core::model::record_stamps::content_value(record)?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("record does not serialize to an object"))?;
    let content = review_families::by_kind(kind).content_fields;
    let filtered = object
        .iter()
        .filter(|(field, _)| content.contains(&field.as_str()))
        .map(|(field, value)| (field.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&filtered)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{changed_fields, changes_revision, content_digest};
    use crate::cache::review_families::REVIEW_FAMILIES;
    use serde_json::json;

    #[test]
    fn every_kind_separates_review_content_from_lifecycle() {
        for family in REVIEW_FAMILIES {
            let content = family.content_fields[0];
            let Some(lifecycle) = family
                .lifecycle_fields
                .iter()
                .find(|field| !matches!(**field, "schema_version" | "scope_id" | "id"))
                .copied()
            else {
                continue;
            };
            assert!(changes_revision(family.kind, &[content.to_string()]));
            assert!(!changes_revision(family.kind, &[lifecycle.to_string()]));
            let before = json!({(content):"A", (lifecycle):"old"});
            let after = json!({(content):"A", (lifecycle):"new"});
            assert_eq!(
                content_digest(family.kind, &before).unwrap(),
                content_digest(family.kind, &after).unwrap()
            );
            let changed = changed_fields(family.kind, &before, &after).unwrap();
            assert!(changed.is_empty() || changed == [lifecycle.to_string()]);
        }
    }
}
