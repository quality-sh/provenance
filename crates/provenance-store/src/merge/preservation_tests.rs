//! Pins the stored-row contract of the merge write: untouched rows keep their
//! stored JSON line bytes, an adopted row keeps the line bytes of the side
//! that moved it, and a record no side stored is refused.

use super::{ensure_unique_ids, preserved_lines, read_jsonl_rows, StoredRow};

#[test]
fn rows_carry_the_stored_line_exactly_as_written() {
    let directory = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(directory.path().join("rule.jsonl")).unwrap();
    let stored = r#"{ "id" : "rule_one" , "note":"kept, as stored" }"#;
    std::fs::write(&path, format!("{stored}\n")).unwrap();

    let rows = read_jsonl_rows(&path).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].line, stored);
    assert_eq!(rows[0].record["id"], "rule_one");
}

#[test]
fn an_untouched_row_keeps_our_line_and_an_adopted_row_keeps_theirs() {
    // Both sides hold the shared record with the same value but a different
    // stored spelling. The merge keeps our spelling: the record is outside
    // the merge's targets, and our line is what our stored shard holds.
    let ours_shared = StoredRow {
        line: r#"{"id":"rule_shared","note" : "ours kept this"}"#.into(),
        record: serde_json::json!({"id":"rule_shared","note":"ours kept this"}),
        line_number: 1,
    };
    let theirs_shared = StoredRow {
        line: r#"{"note":"ours kept this","id":"rule_shared"}"#.into(),
        record: ours_shared.record.clone(),
        line_number: 1,
    };
    let adopted = StoredRow {
        line: r#"{"id":"rule_added","note":"theirs moved it in"}"#.into(),
        record: serde_json::json!({"id":"rule_added","note":"theirs moved it in"}),
        line_number: 2,
    };

    let shared_record = ours_shared.record.clone();
    let added_record = adopted.record.clone();

    let lines = preserved_lines(
        &[ours_shared],
        &[theirs_shared, adopted],
        &[added_record, shared_record],
    )
    .unwrap();

    assert_eq!(
        lines,
        vec![
            r#"{"id":"rule_added","note":"theirs moved it in"}"#,
            r#"{"id":"rule_shared","note" : "ours kept this"}"#,
        ]
    );
}

#[test]
fn a_record_no_side_stored_is_refused_instead_of_reencoded() {
    let error = preserved_lines(
        &[],
        &[StoredRow {
            line: r#"{"id":"rule_theirs"}"#.into(),
            record: serde_json::json!({"id":"rule_theirs"}),
            line_number: 1,
        }],
        &[serde_json::json!({"id":"rule_ghost"})],
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("rule_ghost"), "{error}");
    assert!(error.contains("matches no stored line"), "{error}");
}

#[test]
fn a_side_with_one_id_on_two_rows_is_located_at_the_second_row() {
    let directory = tempfile::tempdir().unwrap();
    let path = camino::Utf8Path::from_path(directory.path())
        .unwrap()
        .join("ours.jsonl");
    std::fs::write(
        &path,
        "{\"id\":\"rule_a\"}\n\n{\"id\":\"rule_a\",\"n\":2}\n",
    )
    .unwrap();
    let rows = read_jsonl_rows(&path).unwrap();
    let shard = camino::Utf8Path::new(".provenance/state/scopes/default/rules/rule.jsonl");

    let error = ensure_unique_ids(shard, "ours", "id", &rows).unwrap_err();

    assert_eq!(
        error.to_string(),
        "scopes/default/rules/rule.jsonl line 3: ours holds record id rule_a at line 1 and at line 3"
    );
}
