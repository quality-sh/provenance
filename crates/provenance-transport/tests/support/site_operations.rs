//! The operations that the review page calls, apart from record history,
//! run in one order on one host: for each of the six record kinds the list,
//! the record, the Discussions and their messages, a new Discussion, a
//! reply, a status change, a withdrawal and a decision; then the creation
//! and the change of a Requirement, the document of a Requirement and the
//! repository metadata. That is 58 operations, 25 of them reads.
//!
//! Each answer keeps its status and its body, without the projection stamp,
//! with a cursor as whether a next page exists, and with `root` as
//! `<repo>`. A path that holds a generated id names it by its place.

use super::catch_up_steps::{Answer, Surface};
use super::resource_http::call_with_headers;
use super::transcript::{named_answers, Names};
use provenance_store::operations::LocalRepository;
use provenance_store::repository::Repository;
use provenance_transport::{OpenCredential, StatementHost};
use serde_json::{json, Value};
use std::sync::Arc;

/// The HTTP routes of a host that serves one opened repository, as the
/// review host binds it.
pub struct Http(pub StatementHost);

impl Http {
    /// A host over `repository` under the target name `selected`.
    pub fn over(repository: Repository) -> Self {
        let access = LocalRepository::bind(repository, "selected").unwrap();
        Self(StatementHost::with_access(Arc::new(access)).with_credential(Arc::new(OpenCredential)))
    }
}

impl Surface for Http {
    fn call<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        data: Option<Value>,
        headers: &'a [(&'a str, &'a str)],
    ) -> Answer<'a> {
        let body = data.map(|data| json!({ "data": data }));
        Box::pin(call_with_headers(&self.0, method, path, body, headers))
    }
}

/// The record kinds, with the id of the record of each kind that the
/// fixture repository holds after `all_kinds`.
pub const KINDS: [(&str, &str, &str); 6] = [
    ("sources", "source", "source_shared"),
    ("requirements", "requirement", "req_shared"),
    ("resolutions", "resolution", "resolution_shared"),
    ("rules", "rule", "rule_shared"),
    ("topics", "topic", "topic_shared"),
    ("questions", "question", "question_shared"),
];

/// The number of calls of [`run`] to read operations: the 25 reads, the
/// new Requirement, and each record again at the end.
pub const READS: usize = 25 + 1 + 6;

/// The calls of one run, with their answers.
pub struct Transcript {
    root: String,
    calls: Vec<Value>,
}

impl Transcript {
    async fn call(
        &mut self,
        surface: &impl Surface,
        method: &str,
        path: &str,
        data: Option<Value>,
        headers: &[(&str, &str)],
        shown: &str,
    ) -> Value {
        let (status, body) = surface.call(method, path, data, headers).await;
        let mut kept = body.clone();
        if let Some(meta) = kept.get_mut("meta").and_then(Value::as_object_mut) {
            meta.remove("stamp");
            if let Some(cursor) = meta.remove("next_cursor") {
                meta.insert("next_cursor".into(), json!(!cursor.is_null()));
            }
        }
        let kept = serde_json::from_str::<Value>(
            &serde_json::to_string(&kept)
                .unwrap()
                .replace(&self.root, "<repo>"),
        )
        .unwrap();
        self.calls.push(json!({
            "call": format!("{method} {shown}"),
            "status": status,
            "body": kept,
        }));
        body
    }

    async fn get(&mut self, surface: &impl Surface, path: &str, shown: &str) -> Value {
        self.call(surface, "GET", path, None, &[], shown).await
    }

    /// The answers with each generated value named in the order it shows.
    pub fn named(&self) -> Vec<Value> {
        named_answers(&mut Names::default(), &self.calls)
    }
}

/// The proposal of the pending submission of the record at `path`, or a
/// proposal id that no submission has.
fn pending(record: &Value) -> String {
    record["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or("proposal_missing")
        .to_owned()
}

/// Runs the operations of the review page on `surface`. `root` is the
/// working tree of the repository behind it.
pub async fn run(surface: &impl Surface, root: &str) -> Transcript {
    let mut out = Transcript {
        root: root.to_owned(),
        calls: Vec::new(),
    };
    for (plural, singular, id) in KINDS {
        kind(&mut out, surface, plural, singular, id).await;
    }
    requirement_writes(&mut out, surface).await;
    out.get(
        surface,
        "/requirements/req_shared/document",
        "/requirements/req_shared/document",
    )
    .await;
    out.call(surface, "GET", "/metadata", None, &[], "/metadata")
        .await;
    for (plural, _, id) in KINDS {
        out.get(
            surface,
            &format!("/{plural}/{id}"),
            &format!("/{plural}/{id}"),
        )
        .await;
    }
    out
}

async fn kind(
    out: &mut Transcript,
    surface: &impl Surface,
    plural: &str,
    singular: &str,
    id: &str,
) {
    let record = format!("/{plural}/{id}");
    let discussions = format!("{record}/discussions");
    out.get(surface, &format!("/{plural}"), &format!("/{plural}"))
        .await;
    let read = out.get(surface, &record, &record).await;
    let started = out
        .call(
            surface,
            "POST",
            &discussions,
            Some(
                json!({"actor":"reviewer", "declared_by":null, "role":"user",
                "body":format!("Discussion of {id}.")}),
            ),
            &[],
            &discussions,
        )
        .await;
    let discussion = started["data"]["discussion_id"]
        .as_str()
        .unwrap_or("discussion_missing")
        .to_owned();
    let thread = format!("{discussions}/{discussion}");
    let shown = format!("{discussions}/{{discussion}}");
    out.get(surface, &discussions, &discussions).await;
    out.call(
        surface,
        "POST",
        &format!("{thread}/messages"),
        Some(
            json!({"actor":"agent", "declared_by":null, "role":"assistant",
            "body":format!("Reply on {id}.")}),
        ),
        &[("if-match", "\"1\"")],
        &format!("{shown}/messages"),
    )
    .await;
    out.get(
        surface,
        &format!("{thread}/messages"),
        &format!("{shown}/messages"),
    )
    .await;
    out.call(
        surface,
        "PATCH",
        &thread,
        Some(json!({"actor":"reviewer", "declared_by":null, "status":"resolved"})),
        &[("if-match", "\"2\"")],
        &shown,
    )
    .await;
    let proposal = pending(&read);
    let submission = format!("{record}/submissions/{proposal}");
    let shown = format!("{record}/submissions/{{proposal}}");
    out.call(
        surface,
        "POST",
        &format!("{submission}/withdraw"),
        Some(json!({"actor":"agent", "declared_by":null, "reason":"Revise it."})),
        &[],
        &format!("{shown}/withdraw"),
    )
    .await;
    out.call(
        surface,
        "POST",
        &format!("{submission}/decide"),
        Some(decision(singular, id)),
        &[],
        &format!("{shown}/decide"),
    )
    .await;
}

fn decision(singular: &str, id: &str) -> Value {
    json!({"actor":{"identity_type":"human","id":"reviewer"},
        "decision":"accepted", "rationale":null,
        "canonical_artifact":{"artifact_type":singular,"artifact_id":id},
        "feedback":null, "declared_by":null})
}

/// Creates a Requirement and withdraws its submission, then changes it and
/// decides the new submission.
// `{proposal}` in a shown call is a placeholder for the generated id.
#[allow(clippy::literal_string_with_formatting_args)]
async fn requirement_writes(out: &mut Transcript, surface: &impl Surface) {
    let created = out
        .call(
            surface,
            "POST",
            "/requirements",
            Some(json!({"actor":"agent", "id":"req_new",
            "statement":"The record req_new is readable.", "status":"discovery",
            "refines":["req_shared"], "depends_on":[], "supersedes":[]})),
            &[],
            "/requirements",
        )
        .await;
    let proposal = created["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or("proposal_missing");
    out.call(
        surface,
        "POST",
        &format!("/requirements/req_new/submissions/{proposal}/withdraw"),
        Some(json!({"actor":"agent", "declared_by":null, "reason":"Revise it."})),
        &[],
        "/requirements/req_new/submissions/{proposal}/withdraw",
    )
    .await;
    let read = out
        .get(surface, "/requirements/req_new", "/requirements/req_new")
        .await;
    let etag = read["data"]["edit"]["etag"]
        .as_str()
        .unwrap_or("missing")
        .to_owned();
    let saved = out
        .call(
            surface,
            "PATCH",
            "/requirements/req_new",
            Some(json!({"actor":"agent", "statement":"The record req_new is new."})),
            &[("if-match", &etag)],
            "/requirements/req_new",
        )
        .await;
    let proposal = saved["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or("proposal_missing");
    out.call(
        surface,
        "POST",
        &format!("/requirements/req_new/submissions/{proposal}/decide"),
        Some(decision("requirement", "req_new")),
        &[],
        "/requirements/req_new/submissions/{proposal}/decide",
    )
    .await;
}
