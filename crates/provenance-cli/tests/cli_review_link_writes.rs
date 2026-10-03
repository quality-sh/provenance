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

fn json_stdin_output(arguments: &[&str], input: &Value) -> Value {
    let output = provenance()
        .args(arguments)
        .write_stdin(input.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn allow_reviewer(repo: &str) {
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
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
            let path_matches = String::from_utf8_lossy(&request[..count])
                .starts_with("GET /review-host-identity HTTP/1.1");
            let (status, response) = if path_matches {
                ("200 OK", body.as_str())
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
fn update_submit_withdraw_and_decide_outputs_keep_the_review_link() {
    let (_directory, repo) = initialized_repo();
    allow_reviewer(&repo);
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
    let expected = format!("{}/?root=req_flow", host.endpoint);
    let created = json_output(&[
        "req_flow",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The review flow keeps its link.",
        "--format",
        "json",
    ]);
    assert_eq!(created["data"]["review_url"], expected);
    let etag = created["data"]["edit"]["etag"].as_str().unwrap();
    let updated = json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "update",
            "--repo",
            &repo,
            "--if-match",
            etag,
            "--stdin",
            "--format",
            "json",
        ],
        &json!({"actor":"agent","description":"Updated review text."}),
    );
    assert_eq!(updated["data"]["review_url"], expected);
    let automatic = updated["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap();
    let withdrawn = json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "submissions",
            automatic,
            "withdraw",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &json!({"actor":"agent","declared_by":null,"reason":null}),
    );
    assert_eq!(withdrawn["data"]["review_url"], expected);
    let submitted = json_stdin_output(
        &[
            "req_flow", "submit", "--repo", &repo, "--stdin", "--format", "json",
        ],
        &json!({
            "actor":"agent", "title":"Review", "summary":"Review the updated record.",
            "source_ids":[], "evidence_references":[], "builds_on":[]
        }),
    );
    assert_eq!(submitted["data"]["review_url"], expected);
    let proposal = submitted["data"]["proposal_id"].as_str().unwrap();
    let decided = json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "submissions",
            proposal,
            "decide",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &json!({
            "actor":{"identity_type":"human","id":"reviewer"}, "decision":"accepted",
            "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_flow"},
            "feedback":null, "declared_by":null
        }),
    );
    assert_eq!(decided["data"]["review_url"], expected);
}
