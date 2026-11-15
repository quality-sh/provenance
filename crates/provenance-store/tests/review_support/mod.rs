use camino::Utf8Path;
use provenance_core::{ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout, repository::Repository, review::SaveRequirement,
    state_store::StateStore,
};
use serde_json::{json, Value};

pub fn fixture() -> (tempfile::TempDir, StateStore) {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    initialization::initialize(&layout, &[]);
    let store = Repository::open(temp.path(), "default").unwrap().store();
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

/// Opens the fixture repository at `root` for the `default` scope.
pub fn repository(root: &Utf8Path) -> Repository {
    Repository::open(root, "default").unwrap()
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
    let output = provenance_store::git_process::git()
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
pub mod initialization;

/// Uses the same manifest plan as repository initialization.
#[allow(dead_code)]
pub fn allow_reviewer(temp: &tempfile::TempDir) {
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    initialization::allow_reviewer(&layout);
}

/// Accepts the pending submission of one record in `default` as the reviewer
/// that [`allow_reviewer`] names, through the public decision.
pub fn accept_pending(store: &StateStore, kind: provenance_core::NodeType, id: &str) {
    let record = StableId::new(id).unwrap();
    let pending = store
        .record_decision_state(&scope(), kind, &record)
        .unwrap()
        .pending
        .unwrap_or_else(|| panic!("{kind:?} {id} has no pending submission"));
    store
        .decide_record_review(
            serde_json::from_value(json!({
                "scope_id": "default", "proposal_id": pending.proposal_id,
                "actor": {"identity_type": "human", "id": "reviewer"},
                "decision": "accepted", "rationale": null,
                "canonical_artifact": {"artifact_type": kind.as_str(), "artifact_id": id},
                "feedback": null, "declared_by": null
            }))
            .unwrap(),
        )
        .unwrap();
}

/// Accepts the pending status request of one record: the request that sets
/// an active or archived status.
pub fn accept_status_request(store: &StateStore, kind: provenance_core::NodeType, id: &str) {
    let record = StableId::new(id).unwrap();
    let state = store
        .record_decision_state(&scope(), kind, &record)
        .unwrap();
    assert!(
        state
            .pending
            .is_some_and(|pending| pending.status_request.is_some()),
        "{kind:?} {id} has no status request"
    );
    accept_pending(store, kind, id);
}
