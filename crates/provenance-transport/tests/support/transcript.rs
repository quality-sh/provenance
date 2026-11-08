//! The read answers of one fixture repository, in a form two repositories
//! with the same canonical state can compare.
//!
//! The comparison of two repositories keeps every value apart from the
//! repository path, the projection stamp and the page cursor. The stamp and
//! the cursors hold the serial of one projection, so an answer keeps only
//! whether a next page exists. Record ids are random from run to run. So the
//! pinned transcript names each generated id, digest and time stamp in the
//! order it first shows.

use super::catch_up_steps::Surface;
use provenance_transport::fixture::records::Repository;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The reads that each step of a characterization records.
pub const READS: &[&str] = &[
    "/requirements",
    "/requirements?exclude_terminal=true",
    "/requirements?query=search&text=record",
    "/sources",
    "/resolutions",
    "/rules",
    "/topics",
    "/questions",
    "/proposals",
    "/assertions",
    "/dispositions",
    "/contributions",
    "/synthesis-packets",
    "/discussion-containers",
    "/messages",
    "/requirements/req_shared",
    "/requirements/req_shared?query=neighbors",
    "/requirements/req_shared?query=trace",
    "/requirements/req_shared?query=impact",
    "/requirements/req_shared/document",
    "/requirements/req_a?query=neighbors",
    "/requirements/req_a?query=trace",
    "/requirements/req_b?query=trace",
    "/rules/rule_a?query=neighbors",
    "/rules/rule_a/evidence",
    "/topics/topic_a?query=neighbors",
    "/questions/q_a?query=neighbors",
    "/sources/source_a?query=impact",
];

#[derive(Default)]
pub struct Names {
    known: BTreeMap<String, String>,
}

impl Names {
    fn name(&mut self, text: &str) -> String {
        if !generated(text) {
            return text.to_owned();
        }
        let next = self.known.len() + 1;
        self.known
            .entry(text.to_owned())
            .or_insert_with(|| format!("<generated {next}>"))
            .clone()
    }
}

/// A UUID with or without a word before it, a hex digest, a signed value
/// or a time stamp.
fn generated(text: &str) -> bool {
    let hex = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_hexdigit());
    let last = text.rsplit('_').next().unwrap_or(text);
    let uuid = last.len() == 36 && last.split('-').map(str::len).eq([8, 4, 4, 4, 12]);
    let digest = text.len() >= 40 && hex(text.trim_start_matches("sha256:"));
    let time = text.len() >= 20
        && text.as_bytes()[4] == b'-'
        && text.as_bytes()[10] == b'T'
        && text.bytes().take(4).all(|b| b.is_ascii_digit());
    uuid || digest || time
}

/// The text with each generated word replaced by `<generated>`.
#[allow(dead_code)]
pub fn without_generated(text: &str) -> String {
    text.split(' ')
        .map(|word| if generated(word) { "<generated>" } else { word })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The answer with each generated value named, and each `created_at`
/// number as `<time>`.
fn named(names: &mut Names, value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(names.name(text)),
        Value::Array(items) => Value::Array(items.iter().map(|item| named(names, item)).collect()),
        Value::Object(fields) => {
            let mut out = Map::new();
            for (key, field) in fields {
                let value = if key == "created_at" && field.is_number() {
                    json!("<time>")
                } else {
                    named(names, field)
                };
                out.insert(key.clone(), value);
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// The value with the repository path replaced in each string.
fn without_root(value: &Value, root: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(root, "<repo>")),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| without_root(item, root)).collect())
        }
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, field)| (key.clone(), without_root(field, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// One read answer without the repository path, the projection stamp and
/// the cursor. Every other value stays as the read gave it.
fn answer(status: u16, body: &Value, root: &str) -> Value {
    let mut body = body.clone();
    if let Some(meta) = body.get_mut("meta").and_then(Value::as_object_mut) {
        meta.remove("stamp");
        if let Some(cursor) = meta.remove("next_cursor") {
            meta.insert("next_cursor".into(), json!(!cursor.is_null()));
        }
    }
    json!({"status": status, "body": without_root(&body, root)})
}

/// Every read of [`READS`] through `surface`, in order, with the values
/// that [`answer`] keeps.
pub async fn read_all(surface: &impl Surface, repo: &Repository) -> Vec<Value> {
    let root = repo.dir.path().to_str().unwrap().to_owned();
    let mut answers = Vec::new();
    for path in READS {
        let (status, body) = surface.call("GET", path, None, &[]).await;
        answers.push(json!({"read": path, "answer": answer(status, &body, &root)}));
    }
    answers
}

/// The answers with each generated value named, for a pinned transcript.
pub fn named_answers(names: &mut Names, answers: &[Value]) -> Vec<Value> {
    answers.iter().map(|answer| named(names, answer)).collect()
}

/// Asserts that each read after a catch-up equals the read after a full
/// rebuild, value for value. A rebuild of a state that validation refuses
/// stores no projection. Then the read after the catch-up must answer from
/// the stored projection and name the refusal.
pub fn assert_same(caught_up: &[Value], rebuilt: &[Value], context: &str) {
    assert_eq!(caught_up.len(), rebuilt.len(), "{context}");
    for (index, (left, right)) in caught_up.iter().zip(rebuilt).enumerate() {
        if right.to_string().contains("no_projection") {
            let refusal = &left["answer"]["body"]["meta"]["freshness_error"];
            assert!(refusal.is_string(), "{context}, read {index}: {left}");
        } else {
            assert_eq!(left, right, "{context}, read {index}");
        }
    }
}

/// A copy of the repository with no projection database, so its first read
/// is a full rebuild.
pub fn copy_without_projection(repo: &Repository) -> Repository {
    let dir = tempfile::tempdir().unwrap();
    let cache = repo.layout.cache_dir();
    let cache = cache
        .strip_prefix(repo.dir.path().to_str().unwrap())
        .unwrap();
    for (path, bytes) in repo.bytes() {
        if path.starts_with(cache) {
            continue;
        }
        let target = dir.path().join(&path);
        if repo.dir.path().join(&path).is_dir() {
            std::fs::create_dir_all(target).unwrap();
        } else {
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
    }
    let layout = provenance_store::layout::ProvenanceLayout::new(dir.path().to_str().unwrap());
    Repository { dir, layout }
}

/// The transcript without what differs from run to run. Record ids such as
/// proposal ids are random, so each generated value becomes one word. A list
/// of records keyed by a generated id is in id order, so it is sorted by its
/// text.
pub fn stable_view(value: &Value) -> Value {
    match value {
        Value::String(text) if text.starts_with("<generated ") => json!("<generated>"),
        Value::Array(items) => {
            let mut items: Vec<Value> = items.iter().map(stable_view).collect();
            let keyed = |item: &Value| item.get("id") == Some(&json!("<generated>"));
            if !items.is_empty() && items.iter().all(keyed) {
                items.sort_by_key(|item| serde_json::to_string(item).unwrap());
            }
            Value::Array(items)
        }
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, field)| (key.clone(), stable_view(field)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// A 64-bit FNV-1a digest of the stable view of the transcript, to pin a
/// long answer list.
pub fn fingerprint(answers: &[Value]) -> String {
    let text = serde_json::to_string(&stable_view(&json!(answers))).unwrap();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}
