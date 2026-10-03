use provenance_macros::verifies;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

struct Host {
    child: Child,
    startup: Value,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let layout =
        provenance_store::layout::ProvenanceLayout::new(directory.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest = provenance_core::Manifest::default_with_scope(
        provenance_core::ScopeId::new("default").unwrap(),
        provenance_core::RepoPathPrefix::new("."),
    );
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    directory
}

fn start(repository: &std::path::Path) -> Host {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "review",
            "--repo",
            repository.to_str().unwrap(),
            "--repository-id",
            "local",
            "--scope",
            "default",
            "--no-open",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        let _ = send.send(line);
    });
    let line = receive.recv_timeout(Duration::from_secs(15)).unwrap();
    Host {
        child,
        startup: serde_json::from_str(&line).unwrap(),
    }
}

fn exchange(host: &Host, code: &str) -> ureq::Response {
    let result = ureq::post(&format!(
        "{}/review-launch/exchange",
        host.startup["endpoint"].as_str().unwrap()
    ))
    .set("Origin", host.startup["endpoint"].as_str().unwrap())
    .set("Content-Type", "application/json")
    .send_string(&serde_json::json!({ "code": code }).to_string());
    match result {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("{error}"),
    }
}

fn launch_code(host: &Host) -> String {
    let url = url::Url::parse(host.startup["url"].as_str().unwrap()).unwrap();
    url.query_pairs()
        .find_map(|(name, value)| (name == "code").then(|| value.into_owned()))
        .unwrap()
}

fn mint_for_scope(host: &Host, scope: &str) -> ureq::Response {
    let body = serde_json::json!({
        "repositoryId": "local",
        "scope": scope,
        "instanceNonce": host.startup["instanceNonce"],
    });
    let result = ureq::post(&format!(
        "{}/review-launch",
        host.startup["endpoint"].as_str().unwrap()
    ))
    .set("Content-Type", "application/json")
    .send_string(&body.to_string());
    match result {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("{error}"),
    }
}

#[test]
#[verifies("rule_review_link_opens_signed_in", examples)]
fn startup_link_exchanges_once_without_disclosing_the_bearer() {
    let repository = repository();
    let host = start(repository.path());
    let url = host.startup["url"].as_str().unwrap();
    let bearer = host.startup["bearer"].as_str().unwrap();

    assert!(!url.contains(bearer));
    assert!(!url.contains("bearer"));
    let code = launch_code(&host);
    let first: Value =
        serde_json::from_str(&exchange(&host, &code).into_string().unwrap()).unwrap();
    assert_eq!(first["bearer"], bearer);

    let replay = exchange(&host, &code);
    assert_eq!(replay.status(), 401);
    let message = replay.into_string().unwrap();
    assert!(message.contains("<record-id> --review-link"), "{message}");
    assert!(!message.contains(bearer));
}

#[test]
fn a_code_from_another_host_is_refused() {
    let first_repository = repository();
    let second_repository = repository();
    let first = start(first_repository.path());
    let second = start(second_repository.path());

    let refusal = exchange(&second, &launch_code(&first));
    assert_eq!(refusal.status(), 401);
    assert!(refusal
        .into_string()
        .unwrap()
        .contains("<record-id> --review-link"));
}

#[test]
fn a_launch_request_for_another_scope_is_refused() {
    let repository = repository();
    let host = start(repository.path());

    let refusal = mint_for_scope(&host, "other");
    assert_eq!(refusal.status(), 403);
    let message = refusal.into_string().unwrap();
    assert!(message.contains("<record-id> --review-link"), "{message}");
    assert!(!message.contains(host.startup["bearer"].as_str().unwrap()));
}
