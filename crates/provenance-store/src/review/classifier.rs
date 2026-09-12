use provenance_core::Requirement;

pub(super) fn changed_fields(
    before: &Requirement,
    after: &Requirement,
) -> anyhow::Result<Vec<String>> {
    let before = provenance_core::model::record_stamps::content_value(before)?;
    let after = provenance_core::model::record_stamps::content_value(after)?;
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
    fields.iter().any(|field| field != "status")
}

#[cfg(test)]
mod tests {
    use super::changed_fields;
    use provenance_core::Requirement;
    use serde_json::json;

    #[test]
    fn stamp_only_changes_are_not_review_content_changes() {
        let value = json!({
            "schema_version": 2,
            "scope_id": "default",
            "id": "req_stamp",
            "statement": "The system stores records.",
            "status": "active"
        });
        let before: Requirement = serde_json::from_value(value.clone()).unwrap();
        let mut after: Requirement = serde_json::from_value(value).unwrap();
        after.created = Some(
            serde_json::from_value(json!({
                "commit": "a".repeat(40),
                "at": "2026-09-12T00:00:00Z"
            }))
            .unwrap(),
        );
        after.updated = Some(
            serde_json::from_value(json!({
                "commit": "b".repeat(40),
                "at": "2026-09-12T01:00:00Z"
            }))
            .unwrap(),
        );

        assert!(changed_fields(&before, &after).unwrap().is_empty());
    }
}
