use camino::Utf8Path;
use serde::{de::DeserializeOwned, Serialize};

pub(super) struct RawRecord {
    line: String,
    known: serde_json::Value,
    unknown: serde_json::Map<String, serde_json::Value>,
    repeated_unknown: Option<String>,
    nested_unknown: Option<String>,
    line_number: usize,
}

impl RawRecord {
    pub(super) fn deserialize<T: DeserializeOwned + Serialize>(
        line: &str,
        value: &serde_json::Value,
        line_number: usize,
    ) -> anyhow::Result<(T, Self)> {
        let mut top_level_unknown = Vec::new();
        let mut repeated_unknown = None;
        let mut nested_unknown = None;
        let mut deserializer = serde_json::Deserializer::from_str(line);
        let record = serde_ignored::deserialize(&mut deserializer, |path| match path {
            serde_ignored::Path::Map {
                parent: serde_ignored::Path::Root,
                key,
            } => {
                if repeated_unknown.is_none() && top_level_unknown.contains(&key) {
                    repeated_unknown = Some(key.clone());
                }
                top_level_unknown.push(key);
            }
            other => {
                if nested_unknown.is_none() {
                    nested_unknown = Some(other.to_string());
                }
            }
        })?;
        let known = serde_json::to_value(&record)?;
        let unknown = top_level_unknown
            .into_iter()
            .filter_map(|key| value.get(&key).cloned().map(|field| (key, field)))
            .collect();
        Ok((
            record,
            Self {
                line: line.to_owned(),
                known,
                unknown,
                repeated_unknown,
                nested_unknown,
                line_number,
            },
        ))
    }

    fn id(&self) -> Option<&str> {
        record_id(&self.known)
    }

    fn has_unknown(&self) -> bool {
        !self.unknown.is_empty() || self.nested_unknown.is_some()
    }

    fn changed_line<T: Serialize>(self, path: &Utf8Path, record: &T) -> anyhow::Result<String> {
        if let Some(field) = self.repeated_unknown {
            anyhow::bail!(
                "{path} line {}: repeated unknown field `{field}` cannot survive a typed record change",
                self.line_number
            );
        }
        if let Some(field) = self.nested_unknown {
            anyhow::bail!(
                "{path} line {}: nested unknown field `{field}` cannot survive a typed record change",
                self.line_number
            );
        }
        append_unknown_fields(serde_json::to_string(record)?, self.unknown)
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
                    lines.push(take(&mut self.raw, index).changed_line(path, record)?);
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

fn append_unknown_fields(
    mut typed: String,
    unknown: serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<String> {
    if unknown.is_empty() {
        return Ok(typed);
    }
    if !typed.ends_with('}') {
        anyhow::bail!("a changed record with unknown fields must serialize as an object");
    }
    typed.pop();
    let mut comma = typed != "{";
    for (key, value) in unknown {
        if comma {
            typed.push(',');
        }
        typed.push_str(&serde_json::to_string(&key)?);
        typed.push(':');
        typed.push_str(&serde_json::to_string(&value)?);
        comma = true;
    }
    typed.push('}');
    Ok(typed)
}
