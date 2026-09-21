use camino::Utf8Path;
use serde::{de::DeserializeOwned, Serialize};

/// One stored line kept beside the record built from it.
///
/// `known` is the record serialized back to JSON, so an unchanged record is
/// recognized by equality. `unknown` holds the exact raw bytes of the
/// top-level members this build does not own, so a changed record can carry
/// them over without re-encoding them. `repeated_member` and
/// `nested_unknown` name stored data that cannot survive a typed change.
pub(super) struct RawRecord {
    line: String,
    known: serde_json::Value,
    unknown: Vec<String>,
    repeated_member: Option<String>,
    nested_unknown: Option<String>,
    line_number: usize,
}

impl RawRecord {
    pub(super) fn deserialize<T: DeserializeOwned + Serialize>(
        line: &str,
        line_number: usize,
    ) -> anyhow::Result<(T, Self)> {
        let mut top_level_unknown = Vec::new();
        let mut nested_unknown = None;
        let mut deserializer = serde_json::Deserializer::from_str(line);
        let record = serde_ignored::deserialize(&mut deserializer, |path| match path {
            serde_ignored::Path::Map {
                parent: serde_ignored::Path::Root,
                key,
            } => top_level_unknown.push(key),
            other => {
                if nested_unknown.is_none() {
                    nested_unknown = Some(other.to_string());
                }
            }
        })?;
        let known = serde_json::to_value(&record)?;
        let scan = top_level_members(line)?;
        let repeated_member = scan.repeated;
        let missing = top_level_unknown
            .iter()
            .filter(|name| !scan.members.iter().any(|member| &member.name == *name))
            .collect::<Vec<_>>();
        anyhow::ensure!(
            missing.is_empty(),
            "line {line_number}: stored field names {missing:?} cannot be read back safely"
        );
        let unknown = scan
            .members
            .into_iter()
            .filter(|member| top_level_unknown.contains(&member.name))
            .map(|member| member.raw)
            .collect();
        Ok((
            record,
            Self {
                line: line.to_owned(),
                known,
                unknown,
                repeated_member,
                nested_unknown,
                line_number,
            },
        ))
    }

    fn id(&self) -> Option<&str> {
        record_id(&self.known)
    }

    const fn has_unknown(&self) -> bool {
        !self.unknown.is_empty()
            || self.repeated_member.is_some()
            || self.nested_unknown.is_some()
    }

    fn changed_line<T: Serialize>(self, path: &Utf8Path, record: &T) -> anyhow::Result<String> {
        if let Some(name) = self.repeated_member {
            anyhow::bail!(
                "{path} line {}: repeated top-level member `{name}` cannot survive a typed record change",
                self.line_number
            );
        }
        if let Some(field) = self.nested_unknown {
            anyhow::bail!(
                "{path} line {}: nested unknown field `{field}` cannot survive a typed record change",
                self.line_number
            );
        }
        if self.unknown.is_empty() {
            return serde_json::to_string(record).map_err(Into::into);
        }
        let mut typed = serde_json::to_string(record)?;
        anyhow::ensure!(
            typed.ends_with('}'),
            "a changed record with stored unknown fields must serialize as an object"
        );
        typed.pop();
        for (position, raw) in self.unknown.iter().enumerate() {
            if position > 0 || typed != "{" {
                typed.push(',');
            }
            typed.push_str(raw);
        }
        typed.push('}');
        Ok(typed)
    }
}

pub(super) struct LoadedRecords<T> {
    records: Vec<T>,
    raw: Vec<Option<RawRecord>>,
}

impl<T> Default for LoadedRecords<T> {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            raw: Vec::new(),
        }
    }
}

impl<T> LoadedRecords<T> {
    pub(super) fn push(&mut self, record: T, raw: RawRecord) {
        self.records.push(record);
        self.raw.push(Some(raw));
    }

    pub(super) fn records(&self) -> &[T] {
        &self.records
    }

    pub(super) const fn records_mut(&mut self) -> &mut Vec<T> {
        &mut self.records
    }
}

impl<T: Serialize> LoadedRecords<T> {
    /// Turns the saved records back into shard lines.
    ///
    /// A record the mutation did not change keeps its raw line, so stored
    /// content this build does not own survives untouched rows byte for byte.
    /// A changed record is matched to its stored row by its unique id, and
    /// the row's top-level unknown members are carried onto the new line. The
    /// write is refused when a changed row holds stored data that cannot be
    /// carried over, before anything is published.
    pub(super) fn into_lines(mut self, path: &Utf8Path) -> anyhow::Result<Vec<String>> {
        let mut lines = Vec::with_capacity(self.records.len());
        let mut added_without_id = false;
        for record in &self.records {
            let known = serde_json::to_value(record)?;
            if let Some(index) = exact_match(&self.raw, &known) {
                lines.push(take(&mut self.raw, index).line);
                continue;
            }
            if let Some(id) = record_id(&known) {
                if let Some(index) = unique_id_match(path, &self.raw, id)? {
                    let raw = take(&mut self.raw, index);
                    lines.push(raw.changed_line(path, record)?);
                    continue;
                }
            } else {
                added_without_id = true;
            }
            lines.push(serde_json::to_string(record)?);
        }
        if added_without_id
            && self
                .raw
                .iter()
                .flatten()
                .any(|raw| raw.id().is_none() && raw.has_unknown())
        {
            anyhow::bail!(
                "{path}: unknown fields cannot survive a typed change to a record without an id"
            );
        }
        Ok(lines)
    }
}

fn exact_match(raw: &[Option<RawRecord>], known: &serde_json::Value) -> Option<usize> {
    raw.iter()
        .position(|candidate| candidate.as_ref().is_some_and(|row| row.known == *known))
}

fn unique_id_match(
    path: &Utf8Path,
    raw: &[Option<RawRecord>],
    id: &str,
) -> anyhow::Result<Option<usize>> {
    let mut matching = raw
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.as_ref().and_then(RawRecord::id) == Some(id))
        .map(|(index, _)| index);
    let first = matching.next();
    if first.is_some() && matching.next().is_some() {
        anyhow::bail!("{path}: duplicate record id `{id}` prevents a safe typed change");
    }
    Ok(first)
}

const fn take(raw: &mut [Option<RawRecord>], index: usize) -> RawRecord {
    raw[index].take().expect("matched raw record is available")
}

fn record_id(value: &serde_json::Value) -> Option<&str> {
    value.get("id").and_then(serde_json::Value::as_str)
}

struct Member {
    name: String,
    raw: String,
}

struct Scan {
    members: Vec<Member>,
    repeated: Option<String>,
}

/// Reads the top-level members of one stored JSON object line.
///
/// The line has already parsed as JSON, so the scan only has to walk it:
/// strings in key position at the first nesting level name members, and a
/// member spans from its name to the comma or closing brace that ends its
/// value. The raw bytes are kept so a rewritten line can carry a member over
/// without re-encoding it.
fn top_level_members(line: &str) -> anyhow::Result<Scan> {
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
    Ok(Scan {
        members,
        repeated,
    })
}

#[cfg(test)]
mod tests {
    use super::top_level_members;

    #[test]
    fn collects_members_in_written_order_with_raw_bytes() {
        let scan = top_level_members(
            r#"{"schema_version":2, "id" : "one","note":"a,b:c}{[#","detail":{"known":1}}"#,
        )
        .unwrap();
        let names = scan
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["schema_version", "id", "note", "detail"]);
        assert!(scan.repeated.is_none());
        assert_eq!(scan.members[1].raw, r#""id" : "one""#);
        assert_eq!(scan.members[2].raw, r#""note":"a,b:c}{[#""#);
    }

    #[test]
    fn detects_a_repeat_written_with_escapes() {
        let scan = top_level_members(r#"{"a":1,"extension":2,"\u0061":3,"a\"b":4,"a\"b":5}"#)
            .unwrap();
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
