use super::{record_from_line, Fields};
use camino::Utf8Path;
use serde_json::{json, Value};

type Loader = fn(&str) -> anyhow::Result<()>;

const LOADERS: [(&str, Loader); 3] = [
    ("an open read", load_open),
    ("a closed read", load_closed),
    ("the read inside a write", load_through_a_write),
];

fn path() -> &'static Utf8Path {
    Utf8Path::new(".provenance/state/scopes/default/requirements/req.jsonl")
}

fn load(line: &str, fields: Fields) -> anyhow::Result<Value> {
    let value = serde_json::from_str(line)?;
    record_from_line(path(), 7, line, value, fields)
}

fn load_open(line: &str) -> anyhow::Result<()> {
    load(line, Fields::Open).map(drop)
}

fn load_closed(line: &str) -> anyhow::Result<()> {
    load(line, Fields::Closed).map(drop)
}

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

    for (loader_name, load_line) in LOADERS {
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
