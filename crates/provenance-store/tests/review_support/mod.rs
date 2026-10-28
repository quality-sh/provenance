use camino::Utf8Path;
use provenance_core::{ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout, review::SaveRequirement, state_store::StateStore,
};
use serde_json::{json, Value};

pub fn fixture() -> (tempfile::TempDir, StateStore) {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    initialization::initialize(&layout, &[]);
    let store = StateStore::new(layout);
    store
        .create_review_requirement(serde_json::from_value(json!({
            "actor":"ben","origin":null,
            "create":{"scope_id":"default","id":"req_a","statement":"The system stores records.","status":"discovery","depends_on":[],"supersedes":[]}
        }))
        .unwrap())
        .unwrap();
    (temp, store)
}

pub fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}
pub fn id() -> StableId {
    StableId::new("req_a").unwrap()
}

pub fn save(store: &StateStore, fields: Value) -> SaveRequirement {
    let mut update = json!({"scope_id":"default", "id":"req_a"});
    let Value::Object(fields) = fields else {
        panic!("update fields must be an object")
    };
    update.as_object_mut().unwrap().extend(fields);
    serde_json::from_value(json!({
        "actor":"ben", "expected_etag":store.requirement_edit_state(&scope(), &id()).unwrap().etag,
        "update":update,"relationships":null
    }))
    .unwrap()
}

/// Runs one Git command in the fixture repository with a fixed identity.
#[allow(dead_code)]
pub fn git(temp: &tempfile::TempDir, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Reviewer",
            "-c",
            "user.email=reviewer@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Commits the saved graph state and returns the commit id.
#[allow(dead_code)]
pub fn commit_state(temp: &tempfile::TempDir, message: &str) -> String {
    if !temp.path().join(".git").exists() {
        git(temp, &["init", "-q"]);
    }
    git(temp, &["add", ".provenance/state"]);
    git(temp, &["commit", "-q", "-m", message]);
    git(temp, &["rev-parse", "HEAD"])
}

#[path = "../support/initialization.rs"]
mod initialization;

/// Uses the same manifest plan as repository initialization.
#[allow(dead_code)]
pub fn allow_reviewer(temp: &tempfile::TempDir) {
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    initialization::allow_reviewer(&layout);
}
