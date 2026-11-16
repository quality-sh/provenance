use super::{record_line_value, record_of_line, top_level_members, untyped_line_value};
use provenance_core::protocol::read_failure::UnparsableStateFile;

#[derive(Debug, serde::Deserialize)]
struct Record {
    id: String,
}

/// The path of the stored lines of these tests.
fn path() -> &'static camino::Utf8Path {
    camino::Utf8Path::new(".provenance/state/scopes/default/rules/rule.jsonl")
}

#[test]
fn collects_members_in_written_order_with_raw_bytes() {
    let scan =
        top_level_members(r#"{"id" : "one","note":"a,b:c}{[#","detail":{"known":1}}"#).unwrap();
    let names = scan
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["id", "note", "detail"]);
    assert!(scan.repeated.is_none());
    assert_eq!(scan.members[0].raw, r#""id" : "one""#);
    assert_eq!(scan.members[1].raw, r#""note":"a,b:c}{[#""#);
}

#[test]
fn detects_a_repeat_written_with_escapes() {
    let scan = top_level_members(r#"{"a":1,"extension":2,"\u0061":3,"a\"b":4,"a\"b":5}"#).unwrap();
    assert_eq!(scan.repeated.as_deref(), Some("a"));
    assert_eq!(scan.members.len(), 5);
}

#[test]
fn escaped_quotes_do_not_end_a_key() {
    let scan = top_level_members(r#"{"a\"b":1,"c":2}"#).unwrap();
    assert_eq!(scan.members[0].name, "a\"b");
    assert_eq!(scan.members[0].raw, r#""a\"b":1"#);
    assert!(scan.repeated.is_none());
}

#[test]
fn an_array_root_has_no_members() {
    let scan = top_level_members(r#"[{"a":1},{"a":2}]"#).unwrap();
    assert!(scan.members.is_empty());
    assert!(scan.repeated.is_none());
}

// The tests below are implementation aids with no Rule: no Rule states the
// order of the faults of one stored line.

/// The line and the message of the refusal `read`.
fn refused<V>(read: anyhow::Result<V>) -> (u64, String) {
    let error = read.err().expect("the line is refused");
    let state = UnparsableStateFile::find(error.as_ref()).unwrap();
    (state.line(), state.message().to_owned())
}

/// The line and the message of the refusal of the line `line` as a `T`.
fn refusal<T: serde::de::DeserializeOwned>(line: &str) -> (u64, String) {
    refused(record_of_line::<T>(path(), 3, line.as_bytes()))
}

/// The line and the message of the refusal of the line `line` of no
/// known record type.
fn untyped_refusal(line: &str) -> (u64, String) {
    refused(untyped_line_value(path(), 3, line.as_bytes()))
}

/// The message of the `retired` refusal of the record `name`.
fn retired(name: &str) -> (u64, String) {
    (
        3,
        format!(
            "{name} contains legacy field `retired`; canonical JSONL requires \
             record-deletion migration 026 before this build can read it"
        ),
    )
}

#[test]
fn a_member_that_the_record_does_not_own_reads() {
    let line = r#"{"id":"rule_a","note":{"kept":1}}"#;
    let record = record_of_line::<Record>(path(), 3, line.as_bytes()).unwrap();
    assert_eq!(record.id, "rule_a");
}

#[test]
fn a_syntax_fault_wins_over_a_repeated_field() {
    assert_eq!(
        refusal::<Record>(r#"{"id":"rule_a","id":"rule_b","#),
        (3, "EOF while parsing a value".to_owned())
    );
}

#[test]
fn the_retired_member_wins_over_a_repeated_field() {
    assert_eq!(
        refusal::<Record>(r#"{"id":"rule_a","id":"rule_b","retired":true}"#),
        retired("record rule_b")
    );
}

#[test]
fn the_retired_member_refuses_a_record_that_parses() {
    assert_eq!(
        refusal::<Record>(r#"{"id":"rule_a","retired":true}"#),
        retired("record rule_a")
    );
}

// The merge driver reads a line of a file of no known record type
// through `untyped_line_value`. The tests below pin that such a line gets
// the checks and the order of a record type.

#[test]
fn the_retired_member_refuses_a_line_of_no_known_type() {
    assert_eq!(
        untyped_refusal(r#"{"id":"rule_a","retired":true}"#),
        retired("record rule_a")
    );
}

#[test]
fn the_retired_member_written_with_an_escape_refuses_each_line() {
    let line = r#"{"id":"rule_a","retir\u0065d":true}"#;
    assert_eq!(untyped_refusal(line), retired("record rule_a"));
    assert_eq!(refusal::<Record>(line), retired("record rule_a"));
}

#[test]
fn a_line_of_no_known_type_refuses_a_repeated_member() {
    assert_eq!(
        untyped_refusal(r#"{"note":1,"id":"rule_a","note":2}"#),
        (3, "duplicate field `note`".to_owned())
    );
}

#[test]
fn a_line_of_no_known_type_gets_the_order_of_a_record() {
    for line in [
        r#"{"id":"rule_a","id":"rule_b","#,
        r#"{"id":"rule_a","id":"rule_b","retired":true}"#,
        r#"{"id":"rule_a","id":"rule_b"}"#,
        r#"{"id":"rule_a"} {}"#,
    ] {
        assert_eq!(untyped_refusal(line), refusal::<Record>(line));
    }
}

#[test]
fn untyped_line_value_gives_the_text_and_the_value_of_a_line() {
    let line = r#"{"id":"rule_a","note":{"kept":1}}"#;
    let (text, value) = untyped_line_value(path(), 3, line.as_bytes()).unwrap();
    assert_eq!(text, line);
    assert_eq!(value, serde_json::json!({"id":"rule_a","note":{"kept":1}}));
}

#[test]
fn record_line_value_gives_the_refusal_of_the_line_reader() {
    for line in [
        r#"{"id":"rule_a","id":"rule_b"}"#,
        r#"{"id":"rule_a","note":1,"note":2}"#,
        r#"{"id":"rule_a","id":"rule_b","retired":true}"#,
    ] {
        assert_eq!(
            refused(record_line_value::<Record>(path(), 3, line.as_bytes())),
            refusal::<Record>(line)
        );
    }
}

#[test]
fn record_line_value_gives_the_text_and_the_value_of_a_line() {
    let line = r#"{"id":"rule_a","note":{"kept":1}}"#;
    let (text, value) = record_line_value::<Record>(path(), 3, line.as_bytes()).unwrap();
    assert_eq!(text, line);
    assert_eq!(value, serde_json::json!({"id":"rule_a","note":{"kept":1}}));
}

#[test]
fn data_after_the_record_refuses_the_line() {
    assert_eq!(
        refusal::<Record>(r#"{"id":"rule_a"} {}"#),
        (3, "trailing characters".to_owned())
    );
}
