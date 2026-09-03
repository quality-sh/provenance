use super::{record_from_line, Fields, IDEATION_LANDING_RECORD_FIELDS, NO_NESTED_RECORDS};
use camino::Utf8Path;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
use serde_json::{json, Value};

const VERSION_RANGE: [u32; 4] = [
    SUPPORTED_SCHEMA_VERSION.0 - 1,
    SUPPORTED_SCHEMA_VERSION.0,
    SUPPORTED_SCHEMA_VERSION.0 + 1,
    u32::MAX,
];
const READABLE_LAYOUT_VERSIONS: [u32; 1] = [SUPPORTED_SCHEMA_VERSION.0];

type Loader = fn(&str) -> anyhow::Result<()>;

const LOADERS: [(&str, Loader); 3] = [
    ("an open read", load_open),
    ("a closed read", load_closed),
    ("the read inside a write", load_through_a_write),
];

fn layout_is_readable_by_oracle(version: u32) -> bool {
    READABLE_LAYOUT_VERSIONS.contains(&version)
}

fn path() -> &'static Utf8Path {
    Utf8Path::new(".provenance/state/scopes/default/requirements/req.jsonl")
}

fn load(line: &str, fields: Fields) -> anyhow::Result<Value> {
    let value = serde_json::from_str(line)?;
    record_from_line(path(), 7, line, value, fields, NO_NESTED_RECORDS)
}

fn load_landing(line: &str) -> anyhow::Result<Value> {
    let value = serde_json::from_str(line)?;
    record_from_line(
        path(),
        7,
        line,
        value,
        Fields::Open,
        IDEATION_LANDING_RECORD_FIELDS,
    )
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
#[verifies("rule_reads_supported_version_only", exhaustion)]
fn only_the_supported_version_loads() {
    for (loader_name, load_line) in LOADERS {
        for version in VERSION_RANGE {
            let line = json!({"schema_version": version, "id": "req_a"}).to_string();
            assert_eq!(
                load_line(&line).is_ok(),
                layout_is_readable_by_oracle(version),
                "the guard and the decision disagree on {loader_name} at version {version}"
            );
        }
    }
}

#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn refusal_names_the_file_the_record_and_both_versions() {
    let future = SUPPORTED_SCHEMA_VERSION.0 + 1;
    let supported = SUPPORTED_SCHEMA_VERSION.0;
    let line =
        json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0 + 1, "id": "req_overtime"}).to_string();
    let message = load(&line, Fields::Open).unwrap_err().to_string();

    assert_eq!(
        message,
        format!("\
.provenance/state/scopes/default/requirements/req.jsonl line 7: \
record req_overtime has schema_version {future}, but this build reads schema_version {supported} only")
    );
}

#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn refusal_says_so_even_when_the_line_carries_no_id() {
    let future = SUPPORTED_SCHEMA_VERSION.0 + 1;
    let line = json!({"schema_version": future}).to_string();
    let message = load(&line, Fields::Open).unwrap_err().to_string();

    assert!(
        message.contains(&format!("record has schema_version {future}")),
        "{message}"
    );
}

#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn a_line_claiming_no_version_is_left_to_its_own_deserializer() {
    let line = json!({"id": "req_a"}).to_string();

    assert!(load(&line, Fields::Open).is_ok());
}

#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn a_nested_landing_record_is_guarded_before_the_batch_loads() {
    let line = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "landing_id": "landing_future",
        "contributions": [{"schema_version": SUPPORTED_SCHEMA_VERSION.0 + 1, "id": "contribution_future"}]
    })
    .to_string();

    let message = load_landing(&line).unwrap_err().to_string();

    assert!(message.contains("record contribution_future"), "{message}");
    assert!(
        message.contains(&format!(
            "has schema_version {}",
            SUPPORTED_SCHEMA_VERSION.0 + 1
        )),
        "{message}"
    );
}

#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn schema_version_inside_record_metadata_is_not_treated_as_a_record_version() {
    let line = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "id": "message_a",
        "ai_metadata": {"schema_version": SUPPORTED_SCHEMA_VERSION.0 + 1},
        "contributions": [{"schema_version": SUPPORTED_SCHEMA_VERSION.0 + 1, "id": "metadata_only"}]
    })
    .to_string();

    assert!(load(&line, Fields::Open).is_ok());
}
