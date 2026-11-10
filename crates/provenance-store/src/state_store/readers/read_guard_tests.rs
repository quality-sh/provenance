use super::record_from_line;
use camino::Utf8Path;
use serde_json::{json, Value};

type Loader = fn(&str) -> anyhow::Result<()>;

/// The write path needs the local disk, so it runs only with `host`.
const LOADERS: &[(&str, Loader)] = &[
    ("a read", load_read),
    #[cfg(feature = "host")]
    ("the read inside a write", load_through_a_write),
];

fn path() -> &'static Utf8Path {
    Utf8Path::new(".provenance/state/scopes/default/requirements/req.jsonl")
}

fn load_read(line: &str) -> anyhow::Result<()> {
    let value = serde_json::from_str(line)?;
    record_from_line::<Value>(path(), 7, value).map(drop)
}

#[cfg(feature = "host")]
fn load_through_a_write(line: &str) -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let shard = camino::Utf8PathBuf::from_path_buf(dir.path().join("req.jsonl"))
        .expect("temporary directory path must be UTF-8");
    std::fs::write(&shard, format!("{line}\n"))?;
    crate::jsonl::mutate_jsonl_locked(
        &shard,
        &shard.with_extension("lock"),
        |records: &mut Vec<Value>| {
            records.clear();
            Ok(())
        },
    )
}

#[test]
fn legacy_retirement_requires_the_canonical_record_deletion_migration() {
    let line = json!({
        "id": "req_retired",
        "retired": true
    })
    .to_string();

    for (loader_name, load_line) in LOADERS.iter().copied() {
        let message = load_line(&line).unwrap_err().to_string();
        assert!(
            message.contains("record req_retired"),
            "{loader_name}: {message}"
        );
        assert!(
            message.contains("legacy field `retired`"),
            "{loader_name}: {message}"
        );
        assert!(
            message.contains("migration 026"),
            "{loader_name}: {message}"
        );
    }
}
