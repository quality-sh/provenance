//! The read answers of one fixture repository, in a form two repositories
//! with the same canonical state can compare.
//!
//! Generated ids and digests get a name in the order the transcript first
//! shows them. The projection stamp and the page cursors hold the serial of
//! one projection, so the transcript keeps only whether a next page exists.

use super::records::Repository;
use super::resource_http::{call, host};
use provenance_transport::StatementHost;
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
    fn name(&mut self, text: &str, root: &str) -> String {
        let text = text.replace(root, "<repo>");
        if !generated(&text) {
            return text;
        }
        let next = self.known.len() + 1;
        self.known
            .entry(text)
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

fn normalize(names: &mut Names, value: &Value, root: &str) -> Value {
    match value {
        Value::String(text) => Value::String(names.name(text, root)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| normalize(names, item, root))
                .collect(),
        ),
        Value::Object(fields) => {
            let mut out = Map::new();
            for (key, field) in fields {
                let value = if key == "created_at" && field.is_number() {
                    json!("<time>")
                } else {
                    normalize(names, field, root)
                };
                out.insert(key.clone(), value);
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// One read answer without its projection stamp and cursor.
fn answer(names: &mut Names, status: u16, body: &Value, root: &str) -> Value {
    let mut body = body.clone();
    if let Some(meta) = body.get_mut("meta").and_then(Value::as_object_mut) {
        meta.remove("stamp");
        if let Some(cursor) = meta.remove("next_cursor") {
            meta.insert("next_cursor".into(), json!(!cursor.is_null()));
        }
    }
    json!({"status": status, "body": normalize(names, &body, root)})
}

/// Every read of [`READS`] through `host`, in order.
pub async fn read_all(host: &StatementHost, repo: &Repository, names: &mut Names) -> Vec<Value> {
    let root = repo.dir.path().to_str().unwrap().to_owned();
    let mut answers = Vec::new();
    for path in READS {
        let (status, body) = call(host, "GET", path, None).await;
        answers.push(json!({"read": path, "answer": answer(names, status, &body, &root)}));
    }
    answers
}

/// A copy of the repository with no projection database, so its first read
/// is a full rebuild.
pub fn copy_without_projection(repo: &Repository) -> Repository {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(
        repo.dir.path(),
        dir.path(),
        &repo.layout.cache_dir().into_std_path_buf(),
    );
    let layout = provenance_store::layout::ProvenanceLayout::new(dir.path().to_str().unwrap());
    Repository { dir, layout }
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path, cache: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.starts_with(cache) {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&path, &target, cache);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// The answers of a fresh rebuild of the same canonical state.
pub async fn rebuilt_answers(repo: &Repository) -> Vec<Value> {
    let copy = copy_without_projection(repo);
    let copy_host = host(&copy, false);
    read_all(&copy_host, &copy, &mut Names::default()).await
}

/// The transcript without what differs from run to run. Record ids such as
/// proposal ids are random, so each generated value becomes one word, and a
/// list of records keyed by a generated id, which reads give in id order,
/// is sorted by its text.
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
