//! The reader of one stored line of a JSONL file. Each read of a state shard,
//! each write that loads a JSONL file and each read of the verification runs
//! cache give the bytes of one line and its number here, so that a fault of a
//! line gets the same refusal from each of them.

use crate::refusal::state_fault;
use crate::state_store::message_ids::ShardLine;
use crate::state_store::readers::located;
use camino::Utf8Path;
use serde::de::DeserializeOwned;

/// The removed member that a stored record must not hold.
const RETIRED: &str = "retired";

/// A stored line that passed the line checks, with its record.
pub(super) struct CheckedLine<'a, T> {
    pub(super) record: T,
    pub(super) text: &'a str,
    pub(super) number: usize,
    /// The names of the top-level members that `T` does not own.
    pub(super) unknown: Vec<String>,
    /// The first nested member that `T` does not own.
    pub(super) nested_unknown: Option<String>,
    /// The top-level members of the line, when a check read them.
    pub(super) members: Option<Scan>,
}

/// `line` without its terminator. A line ends at `\n`, and a `\r` just before
/// that `\n` is part of the terminator.
pub fn without_terminator(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\n")
        .map_or(line, |line| line.strip_suffix(b"\r").unwrap_or(line))
}

/// The lines of the file `contents`, each with its number from 1 and without
/// its terminator, in the split of [`without_terminator`].
pub fn numbered_lines(contents: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    contents
        .split_inclusive(|byte| *byte == b'\n')
        .enumerate()
        .map(|(index, line)| (index + 1, without_terminator(line)))
}

/// The record of the stored line `bytes`, line `number` of `path`, after
/// the checks of [`checked_line`].
pub fn record_of_line<T: DeserializeOwned>(
    path: &Utf8Path,
    number: usize,
    bytes: &[u8],
) -> anyhow::Result<T> {
    checked_line(path, number, bytes).map(|line| line.record)
}

/// The line checks of a stored line of a JSONL file, in this order: the line
/// is UTF-8, it is one JSON value, it holds no `retired` member, it is one
/// record of `T` that holds each field of `T` one time, it holds each member
/// that `T` does not own one time, and the names of those members can be read
/// back. When `T` takes a member of each name, as a JSON value does, the line
/// holds each top-level member one time. A refusal is `invalid_state_file` at
/// the line.
///
/// The line is parsed one time, as `T`. Only a line that fails that parse or
/// can hold a member `retired` is parsed again, as a JSON value, so that a
/// syntax fault and then the `retired` member win over a fault of the record
/// for each `T`.
pub(super) fn checked_line<'a, T: DeserializeOwned>(
    path: &Utf8Path,
    number: usize,
    bytes: &'a [u8],
) -> anyhow::Result<CheckedLine<'a, T>> {
    let text = utf8_text(path, number, bytes)?;
    let at = |error: serde_json::Error| located(path, number, &error);
    let mut unknown = Vec::new();
    let mut nested_unknown = None;
    let record = match typed_record(text, &mut unknown, &mut nested_unknown) {
        Ok(record) => record,
        Err(error) => {
            value_checks(path, number, text)?;
            return Err(at(error));
        }
    };
    if can_hold_retired(text) {
        value_checks(path, number, text)?;
    }
    let members = if unknown.is_empty() && !takes_each_member::<T>() {
        None
    } else {
        let scan = top_level_members(text).map_err(at)?;
        if let Some(name) = &scan.repeated {
            return Err(repeated_member(path, number, name));
        }
        let missing = unknown
            .iter()
            .filter(|name| !scan.members.iter().any(|member| &member.name == *name))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(state_fault(
                &ShardLine::new(path, number),
                format!("stored field names {missing:?} cannot be read back safely"),
            ));
        }
        Some(scan)
    };
    Ok(CheckedLine {
        record,
        text,
        number,
        unknown,
        nested_unknown,
        members,
    })
}

/// The record of `text` with no trailing data. Each top-level member that the
/// record does not own goes to `unknown`, and the first nested one goes to
/// `nested_unknown`.
fn typed_record<T: DeserializeOwned>(
    text: &str,
    unknown: &mut Vec<String>,
    nested_unknown: &mut Option<String>,
) -> Result<T, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let record = serde_ignored::deserialize(&mut deserializer, |ignored| match ignored {
        serde_ignored::Path::Map {
            parent: serde_ignored::Path::Root,
            key,
        } => unknown.push(key),
        other => {
            if nested_unknown.is_none() {
                *nested_unknown = Some(other.to_string());
            }
        }
    })?;
    deserializer.end()?;
    Ok(record)
}

/// Whether `text` can hold a member named `retired`. A JSON string gives the
/// name `retired` only with those bytes or with a `\u` escape, because no other
/// escape gives a letter.
fn can_hold_retired(text: &str) -> bool {
    text.contains(RETIRED) || text.contains("\\u")
}

/// The stored line `bytes`, line `number` of `path`, as its text and as a
/// JSON value, after the line checks of [`checked_line`] for a record of `T`.
/// A caller that does not know the record type of the line gives
/// `serde_json::Value`.
pub fn checked_value<'a, T: DeserializeOwned>(
    path: &Utf8Path,
    number: usize,
    bytes: &'a [u8],
) -> anyhow::Result<(&'a str, serde_json::Value)> {
    let text = checked_line::<T>(path, number, bytes)?.text;
    let value = serde_json::from_str(text).map_err(|error| located(path, number, &error))?;
    Ok((text, value))
}

/// A line with one member, of a name that no record type owns.
const PROBE: &str = r#"{"\u0000":0}"#;

/// Whether `T` takes a member of each name, as a JSON value or a map does.
/// Such a type keeps the last of two equal members and gives no member as one
/// that it does not own, so only a scan of the line finds a repeated member.
fn takes_each_member<T: DeserializeOwned>() -> bool {
    let mut unknown = Vec::new();
    typed_record::<T>(PROBE, &mut unknown, &mut None).is_ok() && unknown.is_empty()
}

/// The line `bytes` as text, or `invalid_state_file` at the line when it is
/// not UTF-8.
fn utf8_text<'a>(path: &Utf8Path, number: usize, bytes: &'a [u8]) -> anyhow::Result<&'a str> {
    std::str::from_utf8(bytes)
        .map_err(|error| state_fault(&ShardLine::new(path, number), error.to_string()))
}

/// The refusal of a line that holds the top-level member `name` two times.
fn repeated_member(path: &Utf8Path, number: usize, name: &str) -> anyhow::Error {
    state_fault(
        &ShardLine::new(path, number),
        format!("duplicate field `{name}`"),
    )
}

/// The checks of `text` as one JSON value: it parses, and it holds no
/// `retired` member.
fn value_checks(path: &Utf8Path, number: usize, text: &str) -> anyhow::Result<()> {
    let value = serde_json::from_str(text).map_err(|error| located(path, number, &error))?;
    ensure_no_legacy_retirement(path, number, &value)
}

/// Refuses a stored record that still holds the removed `retired` field, as
/// `invalid_state_file` at its line.
fn ensure_no_legacy_retirement(
    path: &Utf8Path,
    line_number: usize,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    let Some(record) = value.as_object() else {
        return Ok(());
    };
    if !record.contains_key(RETIRED) {
        return Ok(());
    }
    let name = record
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| "record".to_string(), |id| format!("record {id}"));
    Err(state_fault(
        &ShardLine::new(path, line_number),
        format!(
            "{name} contains legacy field `retired`; canonical JSONL requires \
             record-deletion migration 026 before this build can read it"
        ),
    ))
}

pub(super) struct Member {
    pub(super) name: String,
    pub(super) raw: String,
}

pub(super) struct Scan {
    pub(super) members: Vec<Member>,
    pub(super) repeated: Option<String>,
}

/// Reads the top-level members of one stored JSON object line.
///
/// The line has already parsed as JSON, so the scan only has to walk it:
/// strings in key position at the first nesting level name members, and a
/// member spans from its name to the comma or closing brace that ends its
/// value. The raw bytes are kept so a rewritten line can carry a member over
/// without re-encoding it.
pub(super) fn top_level_members(line: &str) -> Result<Scan, serde_json::Error> {
    let bytes = line.as_bytes();
    let mut depth = 0usize;
    let mut root_object = false;
    let mut in_string = false;
    let mut escaped = false;
    let mut string_start = 0usize;
    let mut expecting_key = false;
    let mut pending: Option<(String, usize)> = None;
    let mut members = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut repeated = None;
    for (index, byte) in bytes.iter().enumerate() {
        if in_string {
            match byte {
                b'\\' => escaped = !escaped,
                b'"' if escaped => escaped = false,
                b'"' => {
                    in_string = false;
                    if depth == 1 && root_object && expecting_key {
                        let name: String = serde_json::from_str(&line[string_start..=index])?;
                        if repeated.is_none() && seen.contains(&name) {
                            repeated = Some(name.clone());
                        }
                        seen.push(name.clone());
                        pending = Some((name, string_start));
                        expecting_key = false;
                    }
                }
                _ => escaped = false,
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                escaped = false;
                string_start = index;
            }
            b'{' | b'[' => {
                depth += 1;
                if depth == 1 {
                    root_object = *byte == b'{';
                    expecting_key = root_object;
                }
            }
            b'}' | b']' => {
                if depth == 1 {
                    if let Some((name, start)) = pending.take() {
                        members.push(Member {
                            name,
                            raw: line[start..index].to_owned(),
                        });
                    }
                }
                depth = depth.saturating_sub(1);
            }
            b',' if depth == 1 => {
                if let Some((name, start)) = pending.take() {
                    members.push(Member {
                        name,
                        raw: line[start..index].to_owned(),
                    });
                }
                expecting_key = root_object;
            }
            _ => {}
        }
    }
    Ok(Scan { members, repeated })
}

#[cfg(test)]
mod tests {
    use super::{checked_value, record_of_line, top_level_members};
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
        let scan =
            top_level_members(r#"{"a":1,"extension":2,"\u0061":3,"a\"b":4,"a\"b":5}"#).unwrap();
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

    /// The line and the message of the refusal of the line `line` as a `T`.
    fn refusal<T: serde::de::DeserializeOwned>(line: &str) -> (u64, String) {
        let error = record_of_line::<T>(path(), 3, line.as_bytes())
            .err()
            .expect("the line is refused");
        let state = UnparsableStateFile::find(error.as_ref()).unwrap();
        (state.line(), state.message().to_owned())
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

    // The merge driver reads a line of a file of no known record type as a
    // JSON value through this reader. The tests below pin that a JSON value
    // gets the checks and the order of a record type.

    #[test]
    fn the_retired_member_refuses_a_line_read_as_a_json_value() {
        assert_eq!(
            refusal::<serde_json::Value>(r#"{"id":"rule_a","retired":true}"#),
            retired("record rule_a")
        );
    }

    #[test]
    fn the_retired_member_written_with_an_escape_refuses_each_type() {
        let line = r#"{"id":"rule_a","retir\u0065d":true}"#;
        assert_eq!(refusal::<serde_json::Value>(line), retired("record rule_a"));
        assert_eq!(refusal::<Record>(line), retired("record rule_a"));
    }

    #[test]
    fn a_line_read_as_a_json_value_refuses_a_repeated_member() {
        assert_eq!(
            refusal::<serde_json::Value>(r#"{"id":"rule_a","id":"rule_b"}"#),
            (3, "duplicate field `id`".to_owned())
        );
    }

    #[test]
    fn a_json_value_gets_the_order_of_a_record() {
        for line in [
            r#"{"id":"rule_a","id":"rule_b","#,
            r#"{"id":"rule_a","id":"rule_b","retired":true}"#,
        ] {
            assert_eq!(refusal::<serde_json::Value>(line), refusal::<Record>(line));
        }
    }

    #[test]
    fn checked_value_gives_the_refusal_of_the_line_reader() {
        for line in [
            r#"{"id":"rule_a","id":"rule_b"}"#,
            r#"{"id":"rule_a","note":1,"note":2}"#,
            r#"{"id":"rule_a","id":"rule_b","retired":true}"#,
        ] {
            let error = checked_value::<Record>(path(), 3, line.as_bytes())
                .err()
                .expect("the line is refused");
            let state = UnparsableStateFile::find(error.as_ref()).unwrap();
            assert_eq!(
                (state.line(), state.message().to_owned()),
                refusal::<Record>(line)
            );
        }
    }

    #[test]
    fn checked_value_gives_the_text_and_the_value_of_a_line() {
        let line = r#"{"id":"rule_a","note":{"kept":1}}"#;
        let (text, value) = checked_value::<Record>(path(), 3, line.as_bytes()).unwrap();
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
}
