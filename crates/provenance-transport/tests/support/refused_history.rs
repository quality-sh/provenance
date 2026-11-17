//! A repository whose Git history holds committed lines that the history
//! read refuses: a record line that is not one JSON value, a record line
//! that holds a top-level member two times, and a Discussion line that is
//! not one JSON value.

use super::records::Repository;
use provenance_core::{ScopeId, StableId};
use provenance_store::review::{CreateReviewRequirement, SaveRequirement};
use serde_json::{json, Value};

/// The record file of the Requirement `req_refused`.
pub const RECORDS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";
/// The Discussion file of the scope of `req_refused`.
pub const DISCUSSIONS: &str = ".provenance/state/scopes/default/threads/discussions.jsonl";

/// The repository, the versions of `req_refused` and the warnings that a
/// history read of `req_refused` gives.
pub struct RefusedHistory {
    pub repo: Repository,
    /// The commit of each version of `req_refused`, oldest first.
    pub versions: Vec<String>,
    /// The warnings of a history read of `req_refused`, in their order.
    pub warnings: Value,
}

fn git(repo: &Repository, args: &[&str]) -> String {
    let output = provenance_store::git_process::git()
        .args(["-c", "user.name=Reviewer"])
        .args(["-c", "user.email=reviewer@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(repo.dir.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Commits the graph state and gives the commit id.
fn commit(repo: &Repository) -> String {
    git(repo, &["add", "--all", ".provenance/state"]);
    git(repo, &["commit", "-q", "-m", "Save"]);
    git(repo, &["rev-parse", "HEAD"])
}

/// Commits `req_refused`, then a record line that is not one JSON value,
/// then a record line that holds `statement` two times, then a new
/// description of `req_refused` with a Discussion line that is not one JSON
/// value, then the Discussion file as it was.
pub fn refused_history() -> RefusedHistory {
    let repo = Repository::new("The shared graph is readable.");
    git(&repo, &["init", "-q"]);
    let store = repo.open().store();
    let scope = ScopeId::new("default").unwrap();
    let id = StableId::new("req_refused").unwrap();
    let create: CreateReviewRequirement = serde_json::from_value(json!({
        "actor":"ben","origin":null,
        "create":{"scope_id":"default","id":"req_refused",
            "statement":"The system stores each record.",
            "status":"discovery","depends_on":[],"supersedes":[]}
    }))
    .unwrap();
    store.create_review_requirement(create).unwrap();
    let created = commit(&repo);

    let records = repo.dir.path().join(RECORDS);
    let text = std::fs::read_to_string(&records).unwrap();
    let line = text.lines().count() + 1;
    std::fs::write(&records, format!("{text}<<<<<<< HEAD\n")).unwrap();
    let marker = commit(&repo);
    let repeated_line = r#"{"id":"req_other","statement":"One.","statement":"Two."}"#;
    std::fs::write(&records, format!("{text}{repeated_line}\n")).unwrap();
    let repeated = commit(&repo);
    std::fs::write(&records, &text).unwrap();

    let save: SaveRequirement = serde_json::from_value(json!({
        "actor":"ben",
        "expected_etag":store.requirement_edit_state(&scope, &id).unwrap().etag,
        "update":{"scope_id":"default","id":"req_refused","description":"Described."},
        "relationships":null
    }))
    .unwrap();
    store.save_requirement(save).unwrap();
    let discussions = repo.dir.path().join(DISCUSSIONS);
    let before = std::fs::read(&discussions).ok();
    let held = String::from_utf8(before.clone().unwrap_or_default()).unwrap();
    let discussion_line = held.lines().count() + 1;
    std::fs::create_dir_all(discussions.parent().unwrap()).unwrap();
    std::fs::write(&discussions, format!("{held}not json\n")).unwrap();
    let described = commit(&repo);
    match before {
        Some(bytes) => std::fs::write(&discussions, bytes).unwrap(),
        None => std::fs::remove_file(&discussions).unwrap(),
    }
    commit(&repo);

    let not_json = "The line is not one JSON value.";
    RefusedHistory {
        repo,
        versions: vec![created, described.clone()],
        warnings: json!([
            {"commit": marker, "path": RECORDS, "line": line, "message": not_json},
            {"commit": repeated, "path": RECORDS, "line": line,
                "message": "The line holds the top-level member \"statement\" two times."},
            {"commit": described, "path": DISCUSSIONS, "line": discussion_line, "message": not_json},
        ]),
    }
}
