use provenance_macros::verifies;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

struct Host {
    child: Child,
    config: Value,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let layout = provenance_store::layout::ProvenanceLayout::new(dir.path().to_str().unwrap());
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
    dir
}

fn start(root: &std::path::Path) -> Host {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "review",
            "--repo",
            root.to_str().unwrap(),
            "--repository-id",
            "A",
            "--scope",
            "default",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        let _ = send.send(line);
    });
    let mut host = Host {
        child,
        config: Value::Null,
    };
    let line = receive
        .recv_timeout(Duration::from_secs(15))
        .expect("host startup line");
    assert!(
        !line.is_empty(),
        "review host must start and publish its configuration"
    );
    host.config = serde_json::from_str(&line).unwrap();
    host
}

fn request(host: &Host, method: &str, path: &str, auth: bool) -> ureq::Request {
    let req = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()
        .request(
            method,
            &format!("{}{path}", host.config["endpoint"].as_str().unwrap()),
        );
    if auth {
        req.set(
            "Authorization",
            &format!("Bearer {}", host.config["bearer"].as_str().unwrap()),
        )
    } else {
        req
    }
}

fn response(result: Result<ureq::Response, ureq::Error>) -> ureq::Response {
    match result {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("{error}"),
    }
}

fn list_discussions(host: &Host) -> ureq::Response {
    response(request(host, "GET", "/discussion-containers", true).call())
}

#[test]
#[verifies("rule_cli_serves_review_assets", examples)]
fn serves_assets_configuration_and_only_the_selected_graph() {
    let repo = repository();
    let host = start(repo.path());
    let index = request(&host, "GET", "/", false).call().unwrap();
    assert_eq!(
        index.header("Content-Type"),
        Some("text/html; charset=utf-8")
    );
    assert!(index.into_string().unwrap().contains("<!doctype html>"));
    assert_eq!(
        request(&host, "GET", "/metadata", true)
            .call()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        response(request(&host, "GET", "/review-config", false).call()).status(),
        401
    );
    let config: Value = serde_json::from_str(
        &request(&host, "GET", "/review-config", true)
            .call()
            .unwrap()
            .into_string()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(config["repositoryId"], "A");
    assert_eq!(config["scope"], "default");
    assert_eq!(
        config["compatibility"],
        json!({"wire":9,"state":2,"review_journal":3,"read_derivation":3})
    );
    assert!(config.get("bearer").is_none());
    assert_eq!(list_discussions(&host).status(), 200);
    for path in [
        "/.provenance/manifest.json",
        "/assets/missing.js",
        "/%2e%2e/Cargo.toml",
    ] {
        assert_eq!(
            response(request(&host, "GET", path, false).call()).status(),
            404
        );
    }
}

#[test]
fn authorization_precedes_body_decode_and_rejects_unrelated_origins() {
    let repo = repository();
    let host = start(repo.path());
    for (method, path) in [
        ("POST", "/requirements/req_example/discussions"),
        ("GET", "/discussion-containers"),
        ("GET", "/requirements/req_absent/document"),
    ] {
        assert_eq!(
            response(request(&host, method, path, false).send_string("invalid")).status(),
            401
        );
        assert_eq!(
            response(
                request(&host, method, path, true)
                    .set("Origin", "https://unrelated.test")
                    .send_string("invalid")
            )
            .status(),
            403
        );
        assert_eq!(
            response(
                request(&host, method, path, true)
                    .set("Origin", "null")
                    .send_string("invalid")
            )
            .status(),
            403
        );
        assert_eq!(
            response(
                request(&host, method, path, true)
                    .set("Host", "unrelated.test")
                    .send_string("invalid")
            )
            .status(),
            403
        );
        assert_eq!(
            response(
                request(&host, method, path, true)
                    .set("Origin", host.config["endpoint"].as_str().unwrap())
                    .send_string("invalid")
            )
            .status(),
            400
        );
    }
    assert_eq!(
        response(
            request(&host, "GET", "/review-config", true)
                .set("Origin", "https://unrelated.test")
                .call()
        )
        .status(),
        403
    );
    let discussions: Value =
        serde_json::from_str(&list_discussions(&host).into_string().unwrap()).unwrap();
    assert_eq!(discussions["data"]["items"], json!([]));
}

#[test]
fn discussion_writes_use_the_bound_scope() {
    let repo = repository();
    let layout = provenance_store::layout::ProvenanceLayout::new(repo.path().to_str().unwrap());
    provenance_store::state_store::StateStore::new(layout)
        .create_requirement(provenance_store::state_store::CreateRequirementInput {
            scope_id: provenance_core::ScopeId::new("default").unwrap(),
            id: provenance_core::StableId::new("req_example").unwrap(),
            statement: "The Requirement accepts review.".into(),
            description: None,
            status: provenance_core::RequirementStatus::Discovery,
            domain_id: None,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let host = start(repo.path());
    let mut body = json!({"data":{
        "actor":"ben", "scope_id":"other", "role":"user",
        "body":"Check this requirement."
    }});
    let post = |body: &Value| {
        response(
            request(&host, "POST", "/requirements/req_example/discussions", true)
                .set("Origin", host.config["endpoint"].as_str().unwrap())
                .set("Content-Type", "application/json")
                .set("Idempotency-Key", "request_review_discussion")
                .send_string(&body.to_string()),
        )
    };
    assert_eq!(post(&body).status(), 400);
    body["data"].as_object_mut().unwrap().remove("scope_id");
    let saved = post(&body);
    assert_eq!(saved.status(), 200);
    let saved: Value = serde_json::from_str(&saved.into_string().unwrap()).unwrap();
    assert_eq!(saved["data"]["request_id"], "request_review_discussion");
    assert_eq!(saved["data"]["actor"], "ben");
    let discussions: Value =
        serde_json::from_str(&list_discussions(&host).into_string().unwrap()).unwrap();
    assert_eq!(discussions["data"]["items"].as_array().unwrap().len(), 1);
}

#[test]
fn refuses_missing_configuration_invalid_repositories_and_busy_ports() {
    assert_cmd::Command::cargo_bin("provenance")
        .unwrap()
        .arg("review")
        .assert()
        .failure();
    let dir = tempfile::tempdir().unwrap();
    assert_cmd::Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "review",
            "--repo",
            dir.path().to_str().unwrap(),
            "--repository-id",
            "A",
            "--scope",
            "default",
        ])
        .assert()
        .failure();
    let repo = repository();
    assert_cmd::Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "review",
            "--repo",
            repo.path().to_str().unwrap(),
            "--repository-id",
            "A",
            "--scope",
            "missing",
        ])
        .assert()
        .failure();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    assert_cmd::Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "review",
            "--repo",
            repo.path().to_str().unwrap(),
            "--repository-id",
            "A",
            "--scope",
            "default",
            "--port",
            &listener.local_addr().unwrap().port().to_string(),
        ])
        .assert()
        .failure();
}

#[cfg(unix)]
#[test]
fn termination_releases_the_listener() {
    use std::{io::Write, net::TcpStream};
    let repo = repository();
    let mut host = start(repo.path());
    let address = host.config["endpoint"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .unwrap()
        .to_owned();
    let mut stalled = TcpStream::connect(&address).unwrap();
    write!(stalled, "POST /requirements/req_example/discussions HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nIdempotency-Key: request_stalled\r\nContent-Length: 1000\r\n\r\n{{", host.config["bearer"].as_str().unwrap()).unwrap();
    let mut partial_headers = TcpStream::connect(&address).unwrap();
    partial_headers.write_all(b"GET / HTTP/1.1\r\nHo").unwrap();
    assert!(Command::new("kill")
        .args(["-TERM", &host.child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = host.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "host shutdown timed out"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(TcpStream::connect(&address).is_err());
    assert!(TcpListener::bind(&address).is_ok());
}

#[test]
fn launch_url_and_responses_do_not_disclose_the_credential() {
    let repo = repository();
    let host = start(repo.path());
    let token = host.config["bearer"].as_str().unwrap();
    assert_eq!(
        host.config["url"],
        format!("{}/", host.config["endpoint"].as_str().unwrap())
    );
    for path in [
        "/",
        "/index.html",
        "/metadata",
        "/review-config",
        "/missing",
    ] {
        let foreign = response(
            request(&host, "GET", path, true)
                .set("Origin", "https://unrelated.test")
                .call(),
        );
        assert_eq!(foreign.status(), 403);
        assert!(foreign.header("Access-Control-Allow-Origin").is_none());
        let reply = response(request(&host, "GET", path, true).call());
        assert_eq!(reply.header("Cache-Control"), Some("no-store"));
        assert_eq!(reply.header("Referrer-Policy"), Some("no-referrer"));
        assert_eq!(reply.header("X-Frame-Options"), Some("DENY"));
        assert!(reply.header("Access-Control-Allow-Origin").is_none());
        assert!(!reply.into_string().unwrap().contains(token));
    }
    for path in [
        format!("/review-config?bearer={token}"),
        format!("/review-config?token={token}"),
    ] {
        assert_eq!(
            response(request(&host, "GET", &path, false).call()).status(),
            401
        );
    }
    assert_eq!(
        response(
            request(&host, "GET", "/review-config", false)
                .set("Cookie", &format!("bearer={token}"))
                .call()
        )
        .status(),
        401
    );
}

#[test]
fn document_root_refusal_follows_target_and_scope_access_checks() {
    let repo = repository();
    let host = start(repo.path());
    for (authorized, expected) in [(false, 401), (true, 409)] {
        let result = response(
            request(
                &host,
                "GET",
                "/requirements/req_absent/document",
                authorized,
            )
            .call(),
        );
        assert_eq!(result.status(), expected);
        if expected == 409 {
            let body: Value = serde_json::from_str(&result.into_string().unwrap()).unwrap();
            assert_eq!(body["error"]["kind"], "document_root_missing");
        }
    }
}
