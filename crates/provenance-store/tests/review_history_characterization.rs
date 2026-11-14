//! Characterization of the record history and evidence reads on a Git
//! history with a merge, and with a Discussion outcome that a later commit
//! records with its record.
//!
//! Implementation aid with no Rule binding: these tests pin the answers of
//! `read_history` and `read_evidence` before the history reads go through
//! the file versions port, so the port cannot change them.

use crate::discussion_support;
use discussion_support::*;
use provenance_core::review::{EvidenceQuery, RecordVersion, ReviewHistoryQuery};
use provenance_core::threads::DiscussionOrigin;
use provenance_core::NodeType;
use provenance_store::{
    layout::ProvenanceLayout,
    operations::read_policy::ReadPolicy,
    review::{read_evidence, read_history},
    state_store::StateStore,
};
use serde_json::json;
use std::cell::Cell;

/// Runs Git in the fixture as `author`, with commit times that step by one
/// minute from a fixed time, so each commit time is known.
struct Git<'a> {
    temp: &'a tempfile::TempDir,
    time: Cell<i64>,
}

impl Git<'_> {
    fn run(&self, author: &str, args: &[&str]) -> String {
        let time = self.time.get();
        self.time.set(time + 60);
        let date = format!("{time} +0000");
        let output = provenance_store::git_process::git()
            .args(["-c", &format!("user.name={author}")])
            .args(["-c", &format!("user.email={author}@example.com")])
            .args(["-c", "commit.gpgsign=false"])
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .args(args)
            .current_dir(self.temp.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// Commits the named paths as `author` and gives the commit id.
    fn commit(&self, author: &str, paths: &[&str], message: &str) -> String {
        let mut add = vec!["add", "--"];
        add.extend_from_slice(paths);
        self.run(author, &add);
        self.run(author, &["commit", "-q", "-m", message]);
        self.run(author, &["rev-parse", "HEAD"])
    }
}

fn create(store: &StateStore, id: &str) {
    store
        .create_review_requirement(
            serde_json::from_value(json!({
                "actor":"ben","origin":null,
                "create":{"scope_id":"default","id":id,"statement":"The system stores records.",
                    "status":"discovery","depends_on":[],"supersedes":[]}
            }))
            .unwrap(),
        )
        .unwrap();
}

fn describe_h(store: &StateStore) {
    let record = provenance_core::StableId::new("req_h").unwrap();
    store
        .save_requirement(
            serde_json::from_value(json!({
                "actor":"ben",
                "expected_etag":store.requirement_edit_state(&scope(), &record).unwrap().etag,
                "update":{"scope_id":"default","id":"req_h","description":"Bob describes req_h."},
                "relationships":null
            }))
            .unwrap(),
        )
        .unwrap();
}

/// The fixture history and the names of its commits, in commit order.
struct Fixture {
    temp: tempfile::TempDir,
    names: Vec<(String, &'static str)>,
    origin: DiscussionOrigin,
}

/// Builds this history of `req_a`:
///
/// - `create`: the record and seven other records, so that two edits on
///   different records merge cleanly.
/// - `start`: a Discussion on the record; the record file does not change.
/// - `alice` on a branch changes the statement; `bob` on `main` describes
///   `req_h`; `merge` takes both and keeps `main`'s other files.
/// - `outcome` commits only the Discussion file after a save from the
///   Discussion; `record` then commits the record with no Discussion change.
/// - The working copy holds one more plain save.
fn history() -> Fixture {
    let (temp, store) = fixture();
    for id in [
        "req_b", "req_c", "req_d", "req_e", "req_f", "req_g", "req_h",
    ] {
        create(&store, id);
    }
    let git = Git {
        temp: &temp,
        time: Cell::new(1_700_000_000),
    };
    git.run("Reviewer", &["init", "-q"]);
    let state = ".provenance/state";
    let mut names = vec![(git.commit("Reviewer", &[state], "Create"), "create")];
    let discussion = start(&store, "Use text A.");
    let origin = DiscussionOrigin {
        discussion_id: discussion.discussion_id.clone(),
        thread_id: discussion.thread_id.clone(),
        message_id: discussion.root_message_id,
    };
    names.push((git.commit("Reviewer", &[state], "Start"), "start"));
    git.run("Reviewer", &["checkout", "-q", "-B", "main"]);
    git.run("Reviewer", &["checkout", "-q", "-b", "feature"]);
    store
        .save_requirement(save(
            &store,
            json!({"statement":"The system stores records for one year."}),
        ))
        .unwrap();
    names.push((
        git.commit("alice", &[state], "alice changes req_a"),
        "alice",
    ));
    git.run("Reviewer", &["checkout", "-q", "main"]);
    describe_h(&store);
    names.push((git.commit("bob", &[state], "bob describes req_h"), "bob"));
    git.run(
        "merger",
        &["merge", "-q", "--no-edit", "-X", "ours", "feature"],
    );
    names.push((git.run("merger", &["rev-parse", "HEAD"]), "merge"));
    store
        .save_requirement_from_discussion(save(&store, json!({"description":"Text A."})), &origin)
        .unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let discussions =
        provenance_store::shards::discussions_path(&ProvenanceLayout::new(root), &scope());
    let discussions = discussions.strip_prefix(root).unwrap().as_str().to_owned();
    names.push((
        git.commit("carol", &[&discussions], "Record the outcome"),
        "outcome",
    ));
    names.push((git.commit("dave", &[state], "Record the change"), "record"));
    store
        .save_requirement(save(&store, json!({"description":"Text B."})))
        .unwrap();
    Fixture {
        temp,
        names,
        origin,
    }
}

impl Fixture {
    fn root(&self) -> &camino::Utf8Path {
        camino::Utf8Path::from_path(self.temp.path()).unwrap()
    }

    fn name(&self, commit: &str) -> String {
        self.names
            .iter()
            .find(|(id, _)| id == commit)
            .map_or_else(|| commit.to_owned(), |(_, name)| (*name).to_owned())
    }

    async fn entries(&self) -> Vec<RecordVersion> {
        let query = ReviewHistoryQuery {
            record_kind: NodeType::Requirement,
            record_id: id(),
            limit: 50,
            cursor: None,
        };
        read_history(&repository(self.root()), ReadPolicy::default(), query)
            .await
            .unwrap()
            .result
            .entries
    }

    /// The record JSON with commit names in place of commit ids and with no
    /// stamp time, which the system clock gives.
    fn record(&self, text: &str) -> String {
        let mut record = serde_json::from_str::<serde_json::Value>(text).unwrap();
        for name in ["created", "updated"] {
            if let Some(stamp) = record.get_mut(name) {
                stamp["at"] = json!("<time>");
                if let Some(commit) = stamp["commit"].as_str() {
                    stamp["commit"] = json!(self.name(commit));
                }
            }
        }
        record.to_string()
    }

    /// One line per version, with commit names in place of commit ids and
    /// the index of the first version with the same revision.
    fn lines(&self, entries: &[RecordVersion]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| {
                let revision = entries
                    .iter()
                    .position(|other| other.revision == entry.revision)
                    .unwrap();
                let origin = entry.origin.as_ref().map(|origin| origin == &self.origin);
                format!(
                    "{} commit={:?} author={:?} at={:?} before={:?} fields={:?} outcome={:?} origin={origin:?} revision={revision}",
                    self.name(entry.id.as_str()),
                    entry.commit.as_deref().map(|commit| self.name(commit)),
                    entry.author,
                    entry.committed_at,
                    entry.before.as_ref().map(|before| self.name(before.as_str())),
                    entry.changed_fields,
                    entry.outcome,
                )
            })
            .collect()
    }
}

#[tokio::test]
async fn history_answers_stay_the_same() {
    let fixture = history();
    let entries = fixture.entries().await;
    assert_eq!(
        fixture.lines(&entries),
        [
            "create commit=Some(\"create\") author=Some(\"Reviewer\") at=Some(1700000120) before=None fields=[\"id\", \"scope_id\", \"statement\", \"status\"] outcome=Created origin=None revision=0",
            "alice commit=Some(\"alice\") author=Some(\"alice\") at=Some(1700000600) before=Some(\"create\") fields=[\"statement\"] outcome=Changed origin=None revision=1",
            "record commit=Some(\"record\") author=Some(\"dave\") at=Some(1700001320) before=Some(\"alice\") fields=[\"description\"] outcome=Changed origin=Some(true) revision=2",
            "working commit=None author=None at=None before=Some(\"record\") fields=[\"description\"] outcome=Changed origin=None revision=3",
        ]
    );
}

#[tokio::test]
async fn evidence_answers_stay_the_same() {
    let fixture = history();
    let entries = fixture.entries().await;
    let mut answers = Vec::new();
    for entry in &entries {
        for field in [None, Some("statement"), Some("description")] {
            for before in [false, true] {
                let query = EvidenceQuery {
                    record_kind: NodeType::Requirement,
                    record_id: id(),
                    entry_id: entry.id.clone(),
                    before,
                    field: field.map(str::to_owned),
                    offset: 0,
                };
                let answer =
                    read_evidence(&repository(fixture.root()), ReadPolicy::default(), query)
                        .await
                        .map_or_else(
                            |error| format!("refused: {error:#}"),
                            |page| {
                                let page = page.result;
                                let text = if field.is_some() {
                                    page.json_text
                                } else {
                                    fixture.record(&page.json_text)
                                };
                                format!(
                                    "{} {text} next={:?}",
                                    fixture.name(page.version.as_str()),
                                    page.next_offset
                                )
                            },
                        );
                answers.push(format!(
                    "{} {field:?} before={before}: {answer}",
                    fixture.name(entry.id.as_str())
                ));
            }
        }
    }
    assert_eq!(
        answers,
        [
            "create None before=false: create {\"id\":\"req_a\",\"scope_id\":\"default\",\"statement\":\"The system stores records.\",\"status\":\"discovery\"} next=None",
            "create None before=true: refused: the addressed resource does not exist",
            "create Some(\"statement\") before=false: create \"The system stores records.\" next=None",
            "create Some(\"statement\") before=true: refused: the addressed resource does not exist",
            "create Some(\"description\") before=false: create null next=None",
            "create Some(\"description\") before=true: refused: the addressed resource does not exist",
            "alice None before=false: alice {\"id\":\"req_a\",\"scope_id\":\"default\",\"statement\":\"The system stores records for one year.\",\"status\":\"discovery\",\"updated\":{\"at\":\"<time>\",\"commit\":\"start\"}} next=None",
            "alice None before=true: create {\"id\":\"req_a\",\"scope_id\":\"default\",\"statement\":\"The system stores records.\",\"status\":\"discovery\"} next=None",
            "alice Some(\"statement\") before=false: alice \"The system stores records for one year.\" next=None",
            "alice Some(\"statement\") before=true: create \"The system stores records.\" next=None",
            "alice Some(\"description\") before=false: alice null next=None",
            "alice Some(\"description\") before=true: create null next=None",
            "record None before=false: record {\"description\":\"Text A.\",\"id\":\"req_a\",\"scope_id\":\"default\",\"statement\":\"The system stores records for one year.\",\"status\":\"discovery\",\"updated\":{\"at\":\"<time>\",\"commit\":\"merge\"}} next=None",
            "record None before=true: alice {\"id\":\"req_a\",\"scope_id\":\"default\",\"statement\":\"The system stores records for one year.\",\"status\":\"discovery\",\"updated\":{\"at\":\"<time>\",\"commit\":\"start\"}} next=None",
            "record Some(\"statement\") before=false: record \"The system stores records for one year.\" next=None",
            "record Some(\"statement\") before=true: alice \"The system stores records for one year.\" next=None",
            "record Some(\"description\") before=false: record \"Text A.\" next=None",
            "record Some(\"description\") before=true: alice null next=None",
            "working None before=false: working {\"description\":\"Text B.\",\"id\":\"req_a\",\"scope_id\":\"default\",\"statement\":\"The system stores records for one year.\",\"status\":\"discovery\",\"updated\":{\"at\":\"<time>\",\"commit\":\"record\"}} next=None",
            "working None before=true: record {\"description\":\"Text A.\",\"id\":\"req_a\",\"scope_id\":\"default\",\"statement\":\"The system stores records for one year.\",\"status\":\"discovery\",\"updated\":{\"at\":\"<time>\",\"commit\":\"merge\"}} next=None",
            "working Some(\"statement\") before=false: working \"The system stores records for one year.\" next=None",
            "working Some(\"statement\") before=true: record \"The system stores records for one year.\" next=None",
            "working Some(\"description\") before=false: working \"Text B.\" next=None",
            "working Some(\"description\") before=true: record \"Text A.\" next=None",
        ]
    );
}
