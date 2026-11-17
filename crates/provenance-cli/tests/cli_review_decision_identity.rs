//! Who can decide and who can withdraw a review submission, through the CLI
//! api command, the HTTP host and MCP. An entry of the manifest field
//! `disposition_actor_ids` names an identity type and an ID, and an actor
//! matches it only when both are equal. The author of a submission or a
//! matching actor can withdraw it; each other actor gets `access_denied`.

use crate::review_host_support;

use provenance_macros::verifies;
use provenance_mcp::Client;
use review_host_support::{follow::create, repository_with_actors, request, response, start};
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

/// The one person in the field.
const LEAD: &str = "lead_reviewer";
/// An actor that is not the author and is not in the field.
const STRANGER: &str = "stranger";
/// The refusal of a decision by an actor that the field does not contain.
const DECISION_REFUSAL: &str = "The manifest field disposition_actor_ids does not contain the \
    actor of a review decision. To change this field, use the command provenance init \
    --disposition-actor-id <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each \
    actor that can make a review decision.";
/// The refusal of a withdrawal by an actor that is not the author and is not in the field.
const WITHDRAWAL_REFUSAL: &str = "The actor of a withdrawal is not the author of the review \
    submission, and the manifest field disposition_actor_ids does not contain the actor. To \
    change this field, use the command provenance init --disposition-actor-id \
    <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each actor that can make a \
    review decision.";

#[derive(Clone, Copy, Debug)]
enum Surface {
    Cli,
    Http,
    Mcp,
}

const SURFACES: [Surface; 3] = [Surface::Cli, Surface::Http, Surface::Mcp];

impl Surface {
    /// The CLI exit code, the HTTP status or the MCP `isError` of a write that lands.
    fn landed(self) -> Value {
        match self {
            Self::Cli => json!(0),
            Self::Http => json!(200),
            Self::Mcp => json!(false),
        }
    }

    /// The CLI exit code, the HTTP status or the MCP `isError` of a refusal.
    fn refused(self, status: u16) -> Value {
        match self {
            Self::Cli => json!(1),
            Self::Http => json!(status),
            Self::Mcp => json!(true),
        }
    }
}

/// The result of one call: its code as [`Surface::landed`] gives it, and the
/// `data` of the answer or its `error`.
struct Answer {
    code: Value,
    body: Value,
}

/// A repository whose field holds the person [`LEAD`], with one pending review
/// submission of `req_review`.
struct Review {
    repo: tempfile::TempDir,
    proposal: String,
    author: String,
}

impl Review {
    fn new() -> Self {
        let repo = repository_with_actors(&[LEAD]);
        let manifest = std::fs::read(repo.path().join(".provenance/state/manifest.json")).unwrap();
        let manifest: Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(
            manifest["disposition_actor_ids"],
            json!([{"identity_type": "human", "id": LEAD}])
        );
        create(repo.path(), "req_review", "requirement", &[]);
        let read = cli(repo.path(), "get", "requirements/req_review", None);
        let proposal = read.body["decision"]["pending"]["proposal_id"]
            .as_str()
            .unwrap_or_else(|| panic!("no pending submission: {}", read.body))
            .to_owned();
        let card = cli(repo.path(), "get", &format!("proposals/{proposal}"), None);
        let author = card.body["actor"]
            .as_str()
            .unwrap_or_else(|| panic!("no author: {}", card.body))
            .to_owned();
        Self {
            repo,
            proposal,
            author,
        }
    }

    fn root(&self) -> &Path {
        self.repo.path()
    }

    /// The review decision state of `req_review`.
    fn decision(&self) -> Value {
        cli(self.root(), "get", "requirements/req_review", None).body["decision"].clone()
    }

    /// Posts `input` to the `action` route of the submission through `surface`.
    fn post(&self, surface: Surface, action: &str, input: &Value) -> Answer {
        let path = format!(
            "requirements/req_review/submissions/{}/{action}",
            self.proposal
        );
        match surface {
            Surface::Cli => cli(self.root(), "post", &path, Some(input)),
            Surface::Http => http(self.root(), &path, input),
            Surface::Mcp => mcp(self.root(), &path, input),
        }
    }
}

fn actor(identity_type: &str, id: &str) -> Value {
    json!({"identity_type": identity_type, "id": id})
}

fn decision(actor: Value) -> Value {
    json!({
        "actor": actor, "decision": "rejected", "rationale": "The statement needs a scope.",
        "canonical_artifact": null, "feedback": null, "declared_by": null
    })
}

fn withdrawal(actor: Value) -> Value {
    json!({"actor": actor, "declared_by": null, "reason": null})
}

fn cli(repo: &Path, method: &str, path: &str, input: Option<&Value>) -> Answer {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.args(["api", path, "--repo", repo.to_str().unwrap()]);
    command.args(["--format", "json", "--method", method]);
    if input.is_some() {
        command.args(["--input", "-"]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    if let Some(input) = input {
        std::io::Write::write_all(&mut stdin, input.to_string().as_bytes()).unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    let code = json!(output.status.code().unwrap());
    if output.status.success() {
        let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        return Answer {
            code,
            body: answer["data"].clone(),
        };
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let envelope: Value = serde_json::from_str(stderr.trim().strip_prefix("Error: ").unwrap())
        .unwrap_or_else(|error| panic!("{error}: {stderr}"));
    Answer {
        code,
        body: envelope["error"].clone(),
    }
}

fn http(repo: &Path, path: &str, input: &Value) -> Answer {
    let host = start(repo);
    let answer = response(
        request(&host, "POST", &format!("/{path}"), true)
            .set("Content-Type", "application/json")
            .send_string(&json!({"data": input}).to_string()),
    );
    let code = json!(answer.status());
    let envelope: Value = serde_json::from_str(&answer.into_string().unwrap()).unwrap();
    let body = if envelope["error"].is_null() {
        envelope["data"].clone()
    } else {
        envelope["error"].clone()
    };
    Answer { code, body }
}

fn mcp(repo: &Path, path: &str, input: &Value) -> Answer {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
            .args(["mcp", "--repo", repo.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let io = tokio::io::join(child.stdout.take().unwrap(), child.stdin.take().unwrap());
        let client = Client::connect(io).await.unwrap();
        let result = client
            .call(
                "api",
                json!({"path": path, "method": "post", "input": input}),
            )
            .await;
        client.cancel().await.unwrap();
        assert!(child.wait().await.unwrap().success());
        let content = &result["structuredContent"];
        let body = if content["error"].is_null() {
            content["data"].clone()
        } else {
            content["error"].clone()
        };
        Answer {
            code: result["isError"].clone(),
            body,
        }
    })
}

#[test]
#[verifies("rule_reviewer_entry_matches_identity_type_and_id", examples)]
fn a_service_with_the_id_of_a_listed_person_cannot_decide() {
    for surface in SURFACES {
        let review = Review::new();
        let before = review.decision();
        let answer = review.post(surface, "decide", &decision(actor("service", LEAD)));
        assert_eq!(
            answer.code,
            surface.refused(403),
            "{surface:?} {}",
            answer.body
        );
        let expected = json!({"kind": "access_denied", "message": DECISION_REFUSAL});
        assert_eq!(answer.body, expected, "{surface:?}");
        assert_eq!(review.decision(), before, "{surface:?}");
    }
}

#[test]
#[verifies("rule_reviewer_entry_matches_identity_type_and_id", examples)]
fn the_listed_person_decides() {
    for surface in SURFACES {
        let review = Review::new();
        let answer = review.post(surface, "decide", &decision(actor("human", LEAD)));
        assert_eq!(answer.code, surface.landed(), "{surface:?} {}", answer.body);
        assert_eq!(
            answer.body["fact"], "rejected",
            "{surface:?} {}",
            answer.body
        );
        assert_eq!(review.decision()["pending"], Value::Null, "{surface:?}");
    }
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
fn the_author_withdraws() {
    for surface in SURFACES {
        let review = Review::new();
        let author = actor("agent", &review.author);
        let answer = review.post(surface, "withdraw", &withdrawal(author));
        assert_eq!(answer.code, surface.landed(), "{surface:?} {}", answer.body);
        assert_eq!(
            answer.body["fact"], "withdrawn",
            "{surface:?} {}",
            answer.body
        );
        let withdrawn = json!([review.proposal]);
        assert_eq!(review.decision()["withdrawn"], withdrawn, "{surface:?}");
    }
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
fn the_listed_person_withdraws() {
    for surface in SURFACES {
        let review = Review::new();
        let answer = review.post(surface, "withdraw", &withdrawal(actor("human", LEAD)));
        assert_eq!(answer.code, surface.landed(), "{surface:?} {}", answer.body);
        assert_eq!(
            answer.body["fact"], "withdrawn",
            "{surface:?} {}",
            answer.body
        );
        let withdrawn = json!([review.proposal]);
        assert_eq!(review.decision()["withdrawn"], withdrawn, "{surface:?}");
    }
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
#[verifies("rule_withdrawal_refusal_is_access_denied", examples)]
fn a_third_actor_cannot_withdraw() {
    for surface in SURFACES {
        let review = Review::new();
        let before = review.decision();
        let answer = review.post(surface, "withdraw", &withdrawal(actor("agent", STRANGER)));
        assert_eq!(
            answer.code,
            surface.refused(403),
            "{surface:?} {}",
            answer.body
        );
        let expected = json!({"kind": "access_denied", "message": WITHDRAWAL_REFUSAL});
        assert_eq!(answer.body, expected, "{surface:?}");
        assert_eq!(review.decision(), before, "{surface:?}");
    }
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
fn a_second_withdrawal_gets_the_submission_conflict() {
    for surface in SURFACES {
        let review = Review::new();
        let author = withdrawal(actor("agent", &review.author));
        let first = review.post(surface, "withdraw", &author);
        assert_eq!(first.code, surface.landed(), "{surface:?} {}", first.body);
        let after = review.decision();
        let second = review.post(surface, "withdraw", &author);
        assert_eq!(
            second.code,
            surface.refused(409),
            "{surface:?} {}",
            second.body
        );
        assert_eq!(
            second.body["kind"], "review_submission_conflict",
            "{surface:?}"
        );
        assert_eq!(review.decision(), after, "{surface:?}");
    }
}
