//! The reader of one stored line of a JSONL file. A write that loads a JSONL
//! file and each read of the verification runs cache give the bytes of one
//! line and its number here, so that a fault of a line gets the same refusal
//! from each of them.

use crate::refusal::state_fault;
use crate::state_store::message_ids::ShardLine;
use crate::state_store::readers::{ensure_no_legacy_retirement, located, parse_line};
use camino::Utf8Path;
use serde::de::DeserializeOwned;

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

/// The record of the stored line `bytes`, line `number` of `path`, after
/// the checks of [`checked_line`].
#[cfg(feature = "host")]
pub fn record_of_line<T: DeserializeOwned>(
    path: &Utf8Path,
    number: usize,
    bytes: &[u8],
) -> anyhow::Result<T> {
    checked_line(path, number, bytes).map(|line| line.record)
}

/// The line checks of a stored line of a JSONL file: the line is UTF-8, it
/// is one JSON value that [`ensure_no_legacy_retirement`] accepts, and it is
/// one record of `T` whose member names can be read back. A refusal is
/// `invalid_state_file` at the line. Each write that loads a JSONL file and
/// each read of the verification runs cache make these checks. The reads of
/// the state shards do not use them.
pub(super) fn checked_line<'a, T: DeserializeOwned>(
    path: &Utf8Path,
    number: usize,
    bytes: &'a [u8],
) -> anyhow::Result<CheckedLine<'a, T>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| state_fault(&ShardLine::new(path, number), error.to_string()))?;
    let value = parse_line(path, number, text)?;
    ensure_no_legacy_retirement(path, number, &value)?;
    let at = |error: serde_json::Error| located(path, number, &error);
    let mut unknown = Vec::new();
    let mut nested_unknown = None;
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let record = serde_ignored::deserialize(&mut deserializer, |ignored| match ignored {
        serde_ignored::Path::Map {
            parent: serde_ignored::Path::Root,
            key,
        } => unknown.push(key),
        other => {
            if nested_unknown.is_none() {
                nested_unknown = Some(other.to_string());
            }
        }
    })
    .map_err(at)?;
    let members = if unknown.is_empty() {
        None
    } else {
        let scan = top_level_members(text).map_err(at)?;
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
    use super::top_level_members;

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
}
