use assert_cmd::Command;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn initialized_repo() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    (directory, repo)
}

fn json_output(arguments: &[&str]) -> Value {
    let output = provenance().args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

struct FakeHost {
    endpoint: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for FakeHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.endpoint.trim_start_matches("http://"));
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn fake_host(identity: &Value) -> FakeHost {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let body = identity.to_string();
    let thread = std::thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let mut request = [0_u8; 2048];
            let count = stream.read(&mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..count]);
            let identity_matches = request.starts_with("GET /review-host-identity HTTP/1.1");
            let launch_matches = request.starts_with("POST /review-launch HTTP/1.1");
            let (status, response) = if identity_matches {
                ("200 OK", body.as_str())
            } else if launch_matches {
                ("200 OK", r#"{"code":"launch-code"}"#)
            } else {
                ("404 Not Found", "{}")
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        }
    });
    FakeHost {
        endpoint,
        stop,
        thread: Some(thread),
    }
}

fn publish_host(repo: &str, stored: &Value) {
    let path = std::path::Path::new(repo).join(".provenance/cache/review-hosts/default.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::set_permissions(
        path.parent().unwrap(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({"hosts":[stored]})).unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn write_output_explains_how_to_start_a_missing_review_host() {
    let (_directory, repo) = initialized_repo();
    let created = json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);

    assert!(created["data"].get("review_url").is_none());
    assert!(created["data"]["review_message"]
        .as_str()
        .unwrap()
        .contains("provenance review"));

    provenance()
        .args([
            "req_readable_link",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The readable output explains how to start review.",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("provenance review --repo"));
}

#[test]
fn write_and_explicit_read_link_to_the_containing_requirement() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);
    let host = fake_host(&json!({
        "repositoryId":"local", "scope":"default", "instanceNonce":"nonce"
    }));
    publish_host(
        &repo,
        &json!({
            "endpoint":host.endpoint, "repositoryId":"local", "scope":"default",
            "instanceNonce":"nonce"
        }),
    );

    let created = json_output(&[
        "rule_link",
        "create",
        "--type",
        "rule",
        "--repo",
        &repo,
        "--statement",
        "The review output includes a link.",
        "--requirement-id",
        "req_link",
        "--format",
        "json",
    ]);
    let expected = format!(
        "{}/?root=req_link&focus=rule_link&code=launch-code",
        host.endpoint
    );
    assert_eq!(created["data"]["review_url"], expected);

    let link = json_output(&[
        "rule_link",
        "get",
        "--repo",
        &repo,
        "--review-link",
        "--format",
        "json",
    ]);
    assert_eq!(link["review_url"], expected);
}

#[test]
fn stale_listener_and_invalid_runtime_records_do_not_produce_links() {
    for stored in [
        json!({"endpoint":"https://127.0.0.1:1234","repositoryId":"local","scope":"default","instanceNonce":"nonce"}),
        json!({"endpoint":"http://127.0.0.1:1234/path","repositoryId":"local","scope":"default","instanceNonce":"nonce"}),
        json!({"endpoint":"http://user@127.0.0.1:1234","repositoryId":"local","scope":"default","instanceNonce":"nonce"}),
        json!({"endpoint":"not a URL","repositoryId":"local","scope":"default","instanceNonce":"nonce"}),
    ] {
        let (_directory, repo) = initialized_repo();
        json_output(&[
            "req_link",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The agent gives the reviewer a safe link.",
            "--format",
            "json",
        ]);
        publish_host(&repo, &stored);
        let link = json_output(&[
            "req_link",
            "get",
            "--repo",
            &repo,
            "--review-link",
            "--format",
            "json",
        ]);
        assert!(link["review_url"].is_null());
        assert!(link["message"]
            .as_str()
            .unwrap()
            .contains("provenance review"));
    }

    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a safe link.",
        "--format",
        "json",
    ]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    publish_host(
        &repo,
        &json!({
            "endpoint":endpoint, "repositoryId":"local", "scope":"default",
            "instanceNonce":"nonce"
        }),
    );
    let link = json_output(&[
        "req_link",
        "get",
        "--repo",
        &repo,
        "--review-link",
        "--format",
        "json",
    ]);
    assert!(link["review_url"].is_null());
}

#[test]
fn identity_mismatches_do_not_produce_links() {
    for identity in [
        json!({"repositoryId":"other","scope":"default","instanceNonce":"nonce"}),
        json!({"repositoryId":"local","scope":"other","instanceNonce":"nonce"}),
        json!({"repositoryId":"local","scope":"default","instanceNonce":"other"}),
    ] {
        let (_directory, repo) = initialized_repo();
        json_output(&[
            "req_link",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The agent gives the reviewer a safe link.",
            "--format",
            "json",
        ]);
        let host = fake_host(&identity);
        publish_host(
            &repo,
            &json!({
                "endpoint":host.endpoint, "repositoryId":"local", "scope":"default",
                "instanceNonce":"nonce"
            }),
        );
        let link = json_output(&[
            "req_link",
            "get",
            "--repo",
            &repo,
            "--review-link",
            "--format",
            "json",
        ]);
        assert!(link["review_url"].is_null());
    }
}

#[test]
fn corrupt_runtime_state_cannot_hide_a_successful_write() {
    let (_directory, repo) = initialized_repo();
    let path = std::path::Path::new(&repo).join(".provenance/cache/review-hosts/default.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"{\"hosts\":[").unwrap();

    let output = provenance()
        .args([
            "req_committed",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The committed write remains successful.",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let created: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(created["data"]["id"], "req_committed");
    assert!(created["data"]["review_url"].is_null());
    assert!(created["data"]["review_message"]
        .as_str()
        .unwrap()
        .contains("provenance review"));
}

#[test]
fn ambiguous_document_decoration_warns_without_hiding_the_write() {
    let (_directory, repo) = initialized_repo();
    for id in ["req_first", "req_second"] {
        json_output(&[
            id,
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The Requirement owns part of the shared Rule.",
            "--format",
            "json",
        ]);
    }

    let output = provenance()
        .args([
            "rule_shared",
            "create",
            "--type",
            "rule",
            "--repo",
            &repo,
            "--statement",
            "The shared Rule belongs to two documents.",
            "--requirement-id",
            "req_first",
            "--requirement-id",
            "req_second",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let created: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(created["data"]["id"], "rule_shared");
    assert!(created["data"].get("review_url").is_none());
    let warning = String::from_utf8_lossy(&output.stderr);
    assert_eq!(warning.lines().count(), 1, "{warning}");
    assert!(warning.contains("write succeeded"), "{warning}");
    assert!(
        warning.contains("multiple Requirement review documents"),
        "{warning}"
    );
}

#[test]
fn explicit_link_read_explains_how_to_start_the_host() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);

    let output = json_output(&[
        "req_link",
        "get",
        "--repo",
        &repo,
        "--review-link",
        "--format",
        "json",
    ]);
    assert!(output["review_url"].is_null());
    assert!(output["message"]
        .as_str()
        .unwrap()
        .contains("provenance review"));
}

#[test]
fn write_without_a_requirement_document_remains_unchanged() {
    let (_directory, repo) = initialized_repo();
    let created = json_output(&[
        "sources",
        "create",
        "--repo",
        &repo,
        "--id",
        "source_without_document",
        "--name",
        "Source without document",
        "--format",
        "json",
    ]);

    assert!(created["data"].get("review_url").is_none());
    assert!(created["data"].get("review_message").is_none());
}
