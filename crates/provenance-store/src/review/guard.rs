//! Only the staged review writer can replace an enrolled Requirement.
use camino::Utf8Path;
use serde::Serialize;
use serde_json::Value;
use std::cell::RefCell;

thread_local! {
    static WRITER: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
}

pub(super) fn with_writer<R>(path: &Utf8Path, id: &str, run: impl FnOnce() -> R) -> R {
    struct Reset(Option<(String, String)>);
    impl Drop for Reset {
        fn drop(&mut self) {
            WRITER.with(|writer| *writer.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(WRITER.with(|writer| {
        writer
            .borrow_mut()
            .replace((path.to_string(), id.to_string()))
    }));
    run()
}

pub fn protect_rows<T: Serialize>(path: &Utf8Path, records: &[T]) -> anyhow::Result<()> {
    if path.file_name() != Some("req.jsonl")
        || path.parent().and_then(Utf8Path::file_name) != Some("requirements")
    {
        return Ok(());
    }
    let before: Vec<Value> = match std::fs::read_to_string(path) {
        Ok(text) => text
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    let after = records
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    for record in before
        .iter()
        .chain(&after)
        .filter(|r| r["schema_version"] == 3)
    {
        let id = record["id"].as_str().unwrap_or_default();
        let allowed = WRITER.with(|writer| {
            writer
                .borrow()
                .as_ref()
                .is_some_and(|(p, i)| p == path.as_str() && i == id)
        });
        if !allowed {
            let old = before.iter().filter(|r| r["id"] == id).collect::<Vec<_>>();
            let new = after.iter().filter(|r| r["id"] == id).collect::<Vec<_>>();
            anyhow::ensure!(
                old.len() == 1 && new.len() == 1 && old == new,
                "enrolled Requirement {id} requires a guarded review save"
            );
        }
    }
    Ok(())
}

pub fn protect_requirements(
    layout: &crate::layout::ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    records: &[provenance_core::Requirement],
) -> anyhow::Result<()> {
    protect_rows(&crate::shards::requirements_path(layout, scope), records)
}
