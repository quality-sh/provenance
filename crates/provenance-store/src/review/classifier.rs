use provenance_core::Requirement;

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
    fields.iter().any(|field| field != "status")
}
