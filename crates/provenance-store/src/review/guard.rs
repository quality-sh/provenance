//! Enrolled records and their Message history require staged review writes.
use camino::Utf8Path;
use serde::Serialize;
use serde_json::Value;
use std::cell::RefCell;

thread_local! {
    static WRITERS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn with_writer<R>(path: &Utf8Path, id: &str, run: impl FnOnce() -> R) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            WRITERS.with(|writers| {
                writers.borrow_mut().pop();
            });
        }
    }
    WRITERS.with(|writers| {
        writers
            .borrow_mut()
            .push((path.to_string(), id.to_string()));
    });
    let _reset = Reset;
    run()
}

pub fn writer_allows(path: &Utf8Path, id: &str) -> bool {
    WRITERS.with(|writers| {
        writers
            .borrow()
            .iter()
            .any(|(p, i)| p == path.as_str() && (i == id || i == "*"))
    })
}

pub fn protect_rows<T: Serialize>(path: &Utf8Path, records: &[T]) -> anyhow::Result<()> {
    if !matches!(
        path.parent().and_then(Utf8Path::file_name),
        Some("requirements" | "threads")
    ) {
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
    if path.parent().and_then(Utf8Path::file_name) == Some("threads")
        && path.file_name() != Some("threads.jsonl")
        && !writer_allows(path, "*")
    {
        let thread_path = path.parent().unwrap().join("threads.jsonl");
        let threads = match std::fs::read_to_string(&thread_path) {
            Ok(text) => text
                .lines()
                .map(serde_json::from_str::<Value>)
                .collect::<Result<Vec<_>, _>>()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        };
        for message in before.iter().chain(&after) {
            if threads
                .iter()
                .any(|t| t["schema_version"] == 3 && t["id"] == message["thread_id"])
            {
                anyhow::ensure!(
                    before
                        .iter()
                        .filter(|m| m["id"] == message["id"])
                        .collect::<Vec<_>>()
                        == after
                            .iter()
                            .filter(|m| m["id"] == message["id"])
                            .collect::<Vec<_>>(),
                    "enrolled Thread requires journaled Message membership"
                );
            }
        }
    }
    for record in before
        .iter()
        .chain(&after)
        .filter(|r| r["schema_version"] == 3)
    {
        let id = record["id"].as_str().unwrap_or_default();
        if !writer_allows(path, id) {
            let old = before.iter().filter(|r| r["id"] == id).collect::<Vec<_>>();
            let new = after.iter().filter(|r| r["id"] == id).collect::<Vec<_>>();
            anyhow::ensure!(
                old.len() == 1 && new.len() == 1 && old == new,
                "enrolled record {id} requires a guarded review save"
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
