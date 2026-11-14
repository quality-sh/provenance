use super::*;
use provenance_core::protocol::read_failure::UnparsableStateFile;
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
struct Record {
    id: &'static str,
}

#[derive(Deserialize, Serialize)]
struct NestedRecord {
    id: String,
    detail: Detail,
}

#[derive(Deserialize, Serialize)]
struct Detail {
    known: String,
}
#[test]
fn writes_newline_terminated_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(dir.path().join("records.jsonl")).unwrap();
    write_jsonl_atomic(&path, &[Record { id: "one" }]).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "{\"id\":\"one\"}\n");
}

#[test]
fn a_merged_record_without_a_stored_row_cannot_replace_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(dir.path()).unwrap();
    let path = root.join("records.jsonl");
    let side = root.join("side.jsonl");
    let original = b"{\"id\":\"original\"}\n";
    std::fs::write(&path, original).unwrap();
    std::fs::write(&side, "{\"id\":\"stored\"}\n").unwrap();
    let rows = crate::merge::read_jsonl_rows(&side).unwrap();
    let records = [serde_json::json!({
        "id": "not-stored"
    })];

    let error = write_preserved_jsonl_atomic(&path, &rows, &[], &records)
        .unwrap_err()
        .to_string();

    assert!(error.contains("not-stored"), "{error}");
    assert!(error.contains("matches no stored line"), "{error}");
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
#[provenance_macros::verifies("rule_publication_section_is_not_entered_twice", examples)]
fn a_raw_write_inside_a_held_section_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repository = crate::cache::tests::fixtures::initialized(dir.path());
    let path = repository.layout().state_dir().join("records.jsonl");
    std::fs::write(&path, "{\"id\":\"stored\"}\n").unwrap();
    let error = crate::publication::with_repository_publication(repository.layout(), |_| {
        write_preserved_jsonl_atomic(&path, &[], &[], &[])
    })
    .unwrap_err();
    assert!(
        error.is::<crate::publication::PublicationHeld>(),
        "{error:#}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"{\"id\":\"stored\"}\n");
}

#[test]
fn persistence_failure_after_mutation_reports_a_write_failure() {
    use crate::write_error::{PublicationStarted, WriteError, WriteFailure};
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(dir.path()).unwrap();
    let path = root.join("records.jsonl");
    let mut mutation_completed = false;
    let error = mutate_jsonl_locked(
        &path,
        &root.join("records.lock"),
        |records: &mut Vec<serde_json::Value>| {
            records.push(serde_json::json!({"id":"one"}));
            // A directory at the destination makes the real atomic replacement fail.
            std::fs::create_dir(&path)?;
            mutation_completed = true;
            Ok(())
        },
    )
    .unwrap_err();
    assert!(mutation_completed);
    let publication = error.downcast_ref::<PublicationStarted>().unwrap();
    let text = format!("{:#}", publication.0);
    assert!(text.starts_with("failed to persist temporary file: "));
    for entry in std::fs::read_dir(root).unwrap() {
        let name = entry.unwrap().file_name();
        assert!(!name.to_string_lossy().starts_with(".tmp"), "{name:?}");
    }
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::WriteFailed { .. }
    ));
}

#[test]
fn mutation_refusal_keeps_its_type_and_does_not_publish_in_memory_changes() {
    use crate::write_error::{PublicationStarted, SourceFailure, WriteError, WriteFailure};
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(dir.path()).unwrap();
    let path = root.join("records.jsonl");
    write_jsonl_atomic(&path, &[serde_json::json!({"id":"one"})]).unwrap();
    let before = std::fs::read(&path).unwrap();
    let error = mutate_jsonl_locked(
        &path,
        &root.join("records.lock"),
        |records: &mut Vec<serde_json::Value>| -> anyhow::Result<()> {
            records.clear();
            Err(SourceFailure::wrap(
                WriteFailure::InvalidCompletion,
                anyhow::anyhow!("invalid completion fixture"),
            ))
        },
    )
    .unwrap_err();
    assert!(error.downcast_ref::<PublicationStarted>().is_none());
    assert_eq!(format!("{error:#}"), "invalid completion fixture");
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::InvalidCompletion
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn unrelated_mutation_keeps_a_nested_unknown_row_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(dir.path()).unwrap();
    let path = root.join("records.jsonl");
    let existing = "  {\"id\":\"one\",\"detail\":{\"extension\":true,\"known\":\"value\"}}  ";
    std::fs::write(&path, format!("{existing}\n")).unwrap();

    mutate_jsonl_locked(
        &path,
        &root.join("records.lock"),
        |records: &mut Vec<NestedRecord>| {
            records.push(NestedRecord {
                id: "two".into(),
                detail: Detail {
                    known: "new".into(),
                },
            });
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        format!("{existing}\n{{\"id\":\"two\",\"detail\":{{\"known\":\"new\"}}}}\n")
    );
}

#[test]
fn mutation_keeps_a_changed_row_with_top_level_unknown_data() {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(dir.path()).unwrap();
    let path = root.join("records.jsonl");
    std::fs::write(
        &path,
        "{\"id\":\"one\",\"detail\":{\"known\":\"value\"},\"extension\":true}\n",
    )
    .unwrap();

    mutate_jsonl_locked(
        &path,
        &root.join("records.lock"),
        |records: &mut Vec<NestedRecord>| {
            records[0].detail.known = "changed".into();
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "{\"id\":\"one\",\"detail\":{\"known\":\"changed\"},\"extension\":true}\n"
    );
}

#[test]
fn mutation_refuses_a_changed_row_with_nested_unknown_data() {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(dir.path()).unwrap();
    let path = root.join("records.jsonl");
    std::fs::write(
        &path,
        "{\"id\":\"one\",\"detail\":{\"known\":\"value\",\"extension\":true}}\n",
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let message = mutate_jsonl_locked(
        &path,
        &root.join("records.lock"),
        |records: &mut Vec<NestedRecord>| {
            records[0].detail.known = "changed".into();
            Ok(())
        },
    )
    .unwrap_err();
    let state = UnparsableStateFile::find(message.as_ref()).unwrap();
    assert_eq!(
        (state.line, state.message.as_str()),
        (
            1,
            "nested unknown field `detail.extension` cannot survive a typed record change"
        )
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn mutation_refuses_a_changed_row_with_a_repeated_unknown_field() {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(dir.path()).unwrap();
    let path = root.join("records.jsonl");
    std::fs::write(
        &path,
        "{\"id\":\"one\",\"detail\":{\"known\":\"value\"},\"extension\":1,\"extension\":2}\n",
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();

    let message = mutate_jsonl_locked(
        &path,
        &root.join("records.lock"),
        |records: &mut Vec<NestedRecord>| {
            records[0].detail.known = "changed".into();
            Ok(())
        },
    )
    .unwrap_err();
    let state = UnparsableStateFile::find(message.as_ref()).unwrap();
    assert_eq!(
        (state.line, state.message.as_str()),
        (
            1,
            "repeated top-level member `extension` cannot survive a typed record change"
        )
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}
