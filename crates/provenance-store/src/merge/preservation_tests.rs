//! Pins the stored-row contract of the merge write: untouched rows keep
//! their exact stored bytes, an adopted row keeps the bytes of the side
//! that moved it, and a record no side stored is refused.

use super::{preserved_lines, read_jsonl_rows, StoredRow};

#[test]
fn rows_carry_the_stored_line_exactly_as_written() {
    let directory = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(directory.path().join("rule.jsonl")).unwrap();
    let stored =
        r#"{ "id" : "rule_one" , "schema_version":2,"note":"kept, as stored" }"#;
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
        line: r#"{"id":"rule_shared","schema_version":2, "note" : "ours kept this"}"#.into(),
        record: serde_json::json!({"id":"rule_shared","schema_version":2,"note":"ours kept this"}),
    };
    let theirs_shared = StoredRow {
        line: r#"{"note":"ours kept this","id":"rule_shared","schema_version":2}"#.into(),
        record: ours_shared.record.clone(),
    };
    let adopted = StoredRow {
        line: r#"{"schema_version":2,"id":"rule_added","note":"theirs moved it in"}"#.into(),
        record: serde_json::json!({"schema_version":2,"id":"rule_added","note":"theirs moved it in"}),
    };

    let lines = preserved_lines(
        &[ours_shared],
        &[theirs_shared, adopted],
        &[adopted.record.clone(), ours_shared.record.clone()],
    )
    .unwrap();

    assert_eq!(
        lines,
        vec![
            r#"{"schema_version":2,"id":"rule_added","note":"theirs moved it in"}"#,
            r#"{"id":"rule_shared","schema_version":2, "note" : "ours kept this"}"#,
        ]
    );
}

#[test]
fn a_record_no_side_stored_is_refused_instead_of_reencoded() {
    let error = preserved_lines(
        &[],
        &[StoredRow {
            line: r#"{"id":"rule_theirs","schema_version":2}"#.into(),
            record: serde_json::json!({"id":"rule_theirs","schema_version":2}),
        }],
        &[serde_json::json!({"id":"rule_ghost","schema_version":2})],
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("rule_ghost"), "{error}");
    assert!(error.contains("matches no stored line"), "{error}");
}
