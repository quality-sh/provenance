use crate::canonical_digest;
use provenance_core::Requirement;

/// The fields a lifecycle-only save may change. They never establish a new
/// revision and never block a decision on the reviewed content.
const LIFECYCLE_FIELDS: [&str; 2] = ["status", "retired"];

pub(super) fn changed_fields(
    before: &Requirement,
    after: &Requirement,
) -> anyhow::Result<Vec<String>> {
    let before = serde_json::to_value(before)?;
    let after = serde_json::to_value(after)?;
    let mut names = before
        .as_object()
        .unwrap()
        .keys()
        .chain(after.as_object().unwrap().keys())
        .filter(|name| name.as_str() != "schema_version")
        .cloned()
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names.retain(|name| before.get(name) != after.get(name));
    Ok(names)
}

pub(super) fn changes_revision(fields: &[String]) -> bool {
    fields
        .iter()
        .any(|field| !LIFECYCLE_FIELDS.contains(&field.as_str()))
}

/// Digests the review-content fields of one record. Lifecycle fields are left
/// out, so a lifecycle-only save keeps the digest a submission bound, and a
/// decision on the reviewed content stays possible after one.
pub(super) fn content_digest(record: &Requirement) -> anyhow::Result<String> {
    let mut value = serde_json::to_value(record)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("record does not serialize to an object"))?;
    object.remove("schema_version");
    for field in LIFECYCLE_FIELDS {
        object.remove(field);
    }
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&value)?,
    ))
}
