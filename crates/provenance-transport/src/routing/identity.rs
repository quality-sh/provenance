use super::invalid;
use provenance_core::protocol::failure::ErasedFailure;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn reject(data: &Value, path: &BTreeMap<String, String>) -> Result<(), ErasedFailure> {
    let Some(object) = data.as_object() else {
        return Ok(());
    };
    if object.get("context").is_some_and(Value::is_object) {
        return Err(invalid(Some("context")));
    }
    let repeated = ["repository", "scope", "scope_id"]
        .into_iter()
        .chain(path.keys().map(String::as_str))
        .find(|name| object.contains_key(*name));
    match repeated {
        Some(field) => Err(invalid(Some(field))),
        None => Ok(()),
    }
}

pub(super) fn node_type(path: &str) -> Result<&'static str, ErasedFailure> {
    match path.split('/').nth(1) {
        Some("sources") => Ok("source"),
        Some("requirements") => Ok("requirement"),
        Some("resolutions") => Ok("resolution"),
        Some("rules") => Ok("rule"),
        Some("domains") => Ok("domain"),
        Some("boundaries") => Ok("boundary"),
        Some("topics") => Ok("topic"),
        Some("questions") => Ok("question"),
        _ => Err(invalid(None)),
    }
}
