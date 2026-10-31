//! The review host refuses a notify registration for another Codex home, a thread outside
//! the repository, a thread name, or another repository or scope, and queues nothing.
#![cfg(target_os = "linux")]

#[path = "review_host_support/codex.rs"]
mod codex;
#[path = "review_host_support/mod.rs"]
mod review_host_support;

use codex::{notify, Codex};
use provenance_macros::verifies;
use review_host_support::{follow::create, repository_with_actors, start_with, Host};
use serde_json::{json, Value};

fn setup() -> (tempfile::TempDir, Codex, Host) {
    let repo = repository_with_actors(&["reviewer"]);
    create(repo.path(), "req_notify", "requirement", &[]);
    let codex = Codex::start();
    let host = start_with(repo.path(), |command| {
        command.arg("--no-open");
        codex.configure(command);
    });
    (repo, codex, host)
}

fn refusal(output: &std::process::Output) -> Value {
    assert!(!output.status.success());
    serde_json::from_slice(&output.stderr).unwrap()
}

#[test]
#[verifies("rule_review_notify_checks_target", examples)]
fn foreign_codex_home_is_refused() {
    let (repo, codex, host) = setup();
    let thread = codex.thread(repo.path());
    let foreign = Codex::start();
    std::fs::copy(
        codex.home.path().join("config.toml"),
        foreign.home.path().join("config.toml"),
    )
    .unwrap();

    let output = notify(&host, &foreign, repo.path(), "req_notify", "codex", &thread);

    let error = refusal(&output);
    assert!(error["error"]["message"]
        .as_str()
        .unwrap()
        .contains("codexHome"));
    assert_eq!(codex.queued(&thread), Vec::<String>::new());
}

#[test]
#[verifies("rule_review_notify_checks_target", examples)]
fn thread_outside_repository_is_refused() {
    let (repo, codex, host) = setup();
    let elsewhere = tempfile::tempdir().unwrap();
    let thread = codex.thread(elsewhere.path());

    let output = notify(&host, &codex, repo.path(), "req_notify", "codex", &thread);

    let error = refusal(&output);
    assert!(error["error"]["message"]
        .as_str()
        .unwrap()
        .contains("does not run in this repository"));
    assert_eq!(codex.queued(&thread), Vec::<String>::new());
}

#[test]
#[verifies("rule_review_notify_checks_target", examples)]
fn thread_name_is_refused() {
    let (repo, codex, host) = setup();
    let thread = codex.thread(repo.path());

    let output = notify(
        &host,
        &codex,
        repo.path(),
        "req_notify",
        "codex",
        "my-session",
    );

    let error = refusal(&output);
    assert!(error["error"]["message"]
        .as_str()
        .unwrap()
        .contains("notify.thread"));
    assert_eq!(codex.queued(&thread), Vec::<String>::new());
}

#[test]
#[verifies("rule_review_notify_checks_target", examples)]
fn repository_or_scope_mismatch_is_refused() {
    let (repo, codex, host) = setup();
    let thread = codex.thread(repo.path());
    let key = std::fs::read_to_string(host.launch_key_path()).unwrap();
    let request = json!({
        "repositoryId": "A", "scope": "default", "instanceNonce": host.config["instanceNonce"],
        "rootId": "req_notify", "limit": 50, "notify": {"client": "codex", "thread": thread},
        "codexHome": codex.home.path(),
    });

    for (field, value) in [("repositoryId", "B"), ("scope", "other")] {
        let mut changed = request.clone();
        changed[field] = json!(value);
        let reply = ureq::post(&format!(
            "{}/review-deliveries",
            host.config["endpoint"].as_str().unwrap()
        ))
        .set("Content-Type", "application/json")
        .set("x-provenance-launch-key", &key)
        .send_string(&changed.to_string());

        let Err(ureq::Error::Status(400, response)) = reply else {
            panic!("{field}: the host must refuse the registration");
        };
        let body: Value = serde_json::from_str(&response.into_string().unwrap()).unwrap();
        assert_eq!(body["error"]["kind"], "invalid_input");
        assert_eq!(body["error"]["field"], field);
    }
    assert_eq!(codex.queued(&thread), Vec::<String>::new());
}
