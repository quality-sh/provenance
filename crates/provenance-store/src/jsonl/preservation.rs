use super::line::{top_level_members, CheckedLine};
use crate::state_store::message_ids::ShardLine;
use crate::state_store::readers::located;
use camino::Utf8Path;
use serde::Serialize;

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
    /// Keeps the checked `line` beside its record, so that a write of the
    /// file can carry the stored line over.
    pub(super) fn keep<T: Serialize>(
        path: &Utf8Path,
        line: CheckedLine<'_, T>,
    ) -> anyhow::Result<(T, Self)> {
        let at = |error: serde_json::Error| located(path, line.number, &error);
        let known = serde_json::to_value(&line.record).map_err(at)?;
        let scan = match line.members {
            Some(scan) => scan,
            None => top_level_members(line.text).map_err(at)?,
        };
        let unknown = scan
            .members
            .into_iter()
            .filter(|member| line.unknown.contains(&member.name))
            .map(|member| member.raw)
            .collect();
        Ok((
            line.record,
            Self {
                line: line.text.to_owned(),
                known,
                unknown,
                repeated_member: scan.repeated,
                nested_unknown: line.nested_unknown,
                line_number: line.number,
            },
        ))
    }

    fn id(&self) -> Option<&str> {
        record_id(&self.known)
    }

    const fn has_unknown(&self) -> bool {
        !self.unknown.is_empty() || self.repeated_member.is_some() || self.nested_unknown.is_some()
    }

    fn changed_line<T: Serialize>(self, path: &Utf8Path, record: &T) -> anyhow::Result<String> {
        if let Some(name) = self.repeated_member {
            return Err(crate::refusal::state_fault(
                &ShardLine::new(path, self.line_number),
                format!("repeated top-level member `{name}` cannot survive a typed record change"),
            ));
        }
        if let Some(field) = self.nested_unknown {
            return Err(crate::refusal::state_fault(
                &ShardLine::new(path, self.line_number),
                format!("nested unknown field `{field}` cannot survive a typed record change"),
            ));
        }
        if self.unknown.is_empty() {
            return serde_json::to_string(record).map_err(Into::into);
        }
        let mut typed = serde_json::to_string(record)?;
        crate::write_error::ensure!(
            Fault,
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

    pub(super) const fn records_mut(&mut self) -> &mut Vec<T> {
        &mut self.records
    }
}

impl<T: Serialize> LoadedRecords<T> {
    /// Turns the saved records back into shard lines.
    ///
    /// A record the mutation did not change keeps its raw line, so stored
    /// JSON line content this build does not own survives untouched rows
    /// unchanged. The writer normalizes line terminators to `\n`.
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
        let kept_without_id = self
            .raw
            .iter()
            .flatten()
            .find(|raw| raw.id().is_none() && raw.has_unknown());
        if let (true, Some(raw)) = (added_without_id, kept_without_id) {
            return Err(crate::refusal::state_fault(
                &ShardLine::new(path, raw.line_number),
                "unknown fields cannot survive a typed change to a record without an id".into(),
            ));
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
    if let Some(second) = matching.next() {
        let line = raw[second]
            .as_ref()
            .expect("the filter keeps only rows that are available")
            .line_number;
        return Err(crate::refusal::state_fault(
            &ShardLine::new(path, line),
            format!("duplicate record id `{id}` prevents a safe typed change"),
        ));
    }
    Ok(first)
}

const fn take(raw: &mut [Option<RawRecord>], index: usize) -> RawRecord {
    raw[index].take().expect("matched raw record is available")
}

fn record_id(value: &serde_json::Value) -> Option<&str> {
    value.get("id").and_then(serde_json::Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::{LoadedRecords, RawRecord};
    use crate::jsonl::line::checked_line;
    use serde::{de::DeserializeOwned, Serialize};

    /// The rows of `lines` as a load of a file keeps them.
    fn load<T: DeserializeOwned + Serialize>(lines: [&str; 2]) -> LoadedRecords<T> {
        let path = camino::Utf8Path::new("records.jsonl");
        let mut loaded = LoadedRecords::default();
        for (index, line) in lines.into_iter().enumerate() {
            let line = checked_line(path, index + 1, line.as_bytes()).unwrap();
            let (record, raw) = RawRecord::keep(path, line).unwrap();
            loaded.push(record, raw);
        }
        loaded
    }

    /// Implementation aid with no Rule: the test builds the stored rows
    /// directly.
    #[test]
    fn a_changed_record_with_a_stored_id_twice_is_located_at_the_second_line() {
        let mut loaded =
            load::<serde_json::Value>([r#"{"id":"one","n":1}"#, r#"{"id":"one","n":2}"#]);
        loaded.records_mut()[0]["n"] = 3.into();
        let error = loaded
            .into_lines(camino::Utf8Path::new("state/scopes/default/records.jsonl"))
            .unwrap_err();
        let state = crate::state_store::readers::StateLineInvalid::find(&error).unwrap();
        assert_eq!(
            (state.line, state.message.as_str()),
            (2, "duplicate record id `one` prevents a safe typed change")
        );
    }

    /// Implementation aid with no Rule: the test builds the stored rows
    /// directly.
    #[test]
    fn a_changed_record_without_an_id_is_located_at_the_kept_row_with_unknown_fields() {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct Row {
            n: u32,
        }
        let mut loaded = load::<Row>([r#"{"n":1}"#, r#"{"n":2,"extension":true}"#]);
        loaded.records_mut()[1].n = 3;
        let error = loaded
            .into_lines(camino::Utf8Path::new("state/scopes/default/records.jsonl"))
            .unwrap_err();
        let state = crate::state_store::readers::StateLineInvalid::find(&error).unwrap();
        assert_eq!(
            (state.path.as_str(), state.line, state.message.as_str()),
            (
                "state/scopes/default/records.jsonl",
                2,
                "unknown fields cannot survive a typed change to a record without an id"
            )
        );
    }
}
