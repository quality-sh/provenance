//! Who can make a review decision, through the CLI api command, the HTTP host
//! and MCP. An entry of the manifest field `disposition_actor_ids` names an
//! identity type and an ID, and an actor matches it only when both are equal.

use crate::review_host_support;
use crate::support::state_files::state_files;

use provenance_macros::verifies;
use review_host_support::surfaces::{Answer, Surface, SURFACES};
use review_host_support::{follow::create, repository_with_actors};
use serde_json::{json, Value};
use std::path::Path;

/// The one person in the field.
pub const LEAD: &str = "lead_reviewer";
/// The next step of a refusal for a person.
pub const PERSON_NEXT_STEP: &str = "To change this field, use the command provenance init \
    --disposition-actor-id <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each \
    person that can make a review decision.";
/// The next step of a refusal for an actor of the identity type `service`.
const SERVICE_NEXT_STEP: &str = "The command provenance init writes only entries with the \
    identity type human. To let an actor with the identity type service make a review decision, \
    add an object with identity_type service and the ID of the actor to this field in the file \
    .provenance/state/manifest.json.";
/// The next step of a refusal for an actor of the identity type `agent`.
pub const AGENT_NEXT_STEP: &str = "The command provenance init writes only entries with the \
    identity type human. To let an actor with the identity type agent make a review decision, add \
    an object with identity_type agent and the ID of the actor to this field in the file \
    .provenance/state/manifest.json.";
/// The cause of the refusal of a decision by an actor that the field does not contain.
const NOT_LISTED: &str =
    "The manifest field disposition_actor_ids does not contain the actor of a review decision.";

/// A repository with one pending review submission of `req_review`.
pub struct Review {
    repo: tempfile::TempDir,
    pub proposal: String,
}

impl Review {
    /// The field holds the persons `actor_ids`. The submission records `author`,
    /// or `authoring` when the write gives no actor.
    pub fn new(actor_ids: &[&str], author: Option<&str>) -> Self {
        let repo = repository_with_actors(actor_ids);
        match author {
            None => create(repo.path(), "req_review", "requirement", &[]),
            Some(author) => {
                let input = json!({
                    "actor": author, "id": "req_review", "statement": "The system keeps each record.",
                    "status": "active", "depends_on": [], "supersedes": []
                });
                let created = Surface::Cli.call(repo.path(), "post", "requirements", Some(&input));
                assert_eq!(created.code, json!(0), "{}", created.text);
            }
        }
        let read = Surface::Cli.call(repo.path(), "get", "requirements/req_review", None);
        let proposal = read.body["decision"]["pending"]["proposal_id"]
            .as_str()
            .unwrap_or_else(|| panic!("no pending submission: {}", read.text))
            .to_owned();
        Self { repo, proposal }
    }

    pub fn root(&self) -> &Path {
        self.repo.path()
    }

    /// Replaces the entries of the field with `entries`, as a person edits the file.
    pub fn set_entries(&self, entries: Value) {
        let path = self.root().join(".provenance/state/manifest.json");
        let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        manifest["disposition_actor_ids"] = entries;
        std::fs::write(&path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    }

    /// The review decision state of `req_review`.
    pub fn decision(&self) -> Value {
        let read = Surface::Cli.call(self.root(), "get", "requirements/req_review", None);
        read.body["decision"].clone()
    }

    /// Posts `input` to the `action` route of the submission through `surface`.
    pub fn post(&self, surface: Surface, action: &str, input: &Value) -> Answer {
        self.post_to(surface, &self.proposal, action, input)
    }

    /// Posts `input` to the `action` route of `proposal` through `surface`.
    pub fn post_to(&self, surface: Surface, proposal: &str, action: &str, input: &Value) -> Answer {
        let path = format!("requirements/req_review/submissions/{proposal}/{action}");
        surface.call(self.root(), "post", &path, Some(input))
    }

    /// Checks that `answer` is the refusal `expected` with the HTTP status
    /// `status`, and that the refusal changed no stored file.
    pub fn assert_refused(
        &self,
        surface: Surface,
        status: u16,
        expected: &Value,
        act: impl FnOnce() -> Answer,
    ) {
        let before = state_files(self.root());
        let answer = act();
        assert_eq!(
            answer.code,
            surface.refused(status),
            "{surface:?} {}",
            answer.text
        );
        assert_eq!(&answer.body, expected, "{surface:?}");
        assert_eq!(state_files(self.root()), before, "{surface:?}");
    }
}

pub fn actor(identity_type: &str, id: &str) -> Value {
    json!({"identity_type": identity_type, "id": id})
}

fn decision(actor: &Value) -> Value {
    json!({
        "actor": actor, "decision": "rejected", "rationale": "The statement needs a scope.",
        "canonical_artifact": null, "feedback": null, "declared_by": null
    })
}

#[test]
#[verifies("rule_reviewer_entry_matches_identity_type_and_id", examples)]
fn a_service_with_the_id_of_a_listed_person_cannot_decide() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], None);
        let message = format!("{NOT_LISTED} {SERVICE_NEXT_STEP}");
        let expected = json!({"kind": "access_denied", "message": message});
        review.assert_refused(surface, 403, &expected, || {
            review.post(surface, "decide", &decision(&actor("service", LEAD)))
        });
    }
}

#[test]
#[verifies("rule_reviewer_entry_matches_identity_type_and_id", examples)]
fn the_listed_person_decides_and_the_name_has_no_effect() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], None);
        review
            .set_entries(json!([{"identity_type": "human", "id": LEAD, "name": "Lead Reviewer"}]));
        let mut named = actor("human", LEAD);
        named["name"] = json!("Another Name");
        let answer = review.post(surface, "decide", &decision(&named));
        assert_eq!(answer.code, surface.landed(), "{surface:?} {}", answer.text);
        assert_eq!(answer.body["fact"], "decided", "{surface:?}");
        assert_eq!(review.decision()["pending"], Value::Null, "{surface:?}");
    }
}

/// A decision that the list matched by type, stored before the entry changed
/// its identity type, makes each check and each write of the scope fail.
#[test]
#[verifies("rule_disposition_actor_allowlist", examples)]
fn a_stored_decision_that_the_list_no_longer_matches_by_type_stops_the_scope() {
    let review = Review::new(&[LEAD], None);
    review.set_entries(json!([actor("agent", "ci_bot")]));
    let decided = review.post(Surface::Cli, "decide", &decision(&actor("agent", "ci_bot")));
    assert_eq!(decided.code, json!(0), "{}", decided.text);
    review.set_entries(json!([actor("human", "ci_bot")]));

    let check = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args(["check", "--repo", review.root().to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(1));
    let refusal = format!("Error: {NOT_LISTED} {AGENT_NEXT_STEP}\n");
    assert_eq!(String::from_utf8_lossy(&check.stderr), refusal);

    let before = state_files(review.root());
    let input = json!({
        "actor": "shaping_agent", "id": "req_other", "statement": "The system keeps each copy.",
        "status": "active", "depends_on": [], "supersedes": []
    });
    let write = Surface::Cli.call(review.root(), "post", "requirements", Some(&input));
    assert_eq!(write.code, json!(1), "{}", write.text);
    let message = format!("{NOT_LISTED} {AGENT_NEXT_STEP}");
    assert_eq!(
        write.body,
        json!({"kind": "access_denied", "message": message})
    );
    assert_eq!(state_files(review.root()), before);
}
