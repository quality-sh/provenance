use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

struct Host {
    child: Child,
    endpoint: String,
    bearer: String,
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
    let line = receive
        .recv_timeout(Duration::from_secs(15))
        .expect("host startup line");
    let config: Value = serde_json::from_str(&line).unwrap();
    Host {
        child,
        endpoint: config["endpoint"].as_str().unwrap().to_owned(),
        bearer: config["bearer"].as_str().unwrap().to_owned(),
    }
}

fn response(result: Result<ureq::Response, ureq::Error>) -> ureq::Response {
    match result {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("{error}"),
    }
}

#[test]
fn body_bearing_refusals_do_not_break_the_next_request() {
    let repo = repository();
    let host = start(repo.path());
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    let body = "invalid".repeat(1024);

    for _ in 0..4 {
        let refusal = response(
            agent
                .post(&format!(
                    "{}/requirements/req_example/discussions",
                    host.endpoint
                ))
                .send_string(&body),
        );
        assert_eq!(refusal.status(), 401);
        refusal.into_string().expect("refusal response body");
    }

    let valid = response(
        agent
            .get(&format!("{}/review-config", host.endpoint))
            .set("Authorization", &format!("Bearer {}", host.bearer))
            .call(),
    );
    assert_eq!(valid.status(), 200);
}

#[test]
fn early_refusals_ask_the_client_to_close_the_connection() {
    let repo = repository();
    let host = start(repo.path());
    let refusal = response(
        ureq::post(&format!(
            "{}/requirements/req_example/discussions",
            host.endpoint
        ))
        .send_string("invalid"),
    );

    assert_eq!(refusal.status(), 401);
    assert_eq!(refusal.header("Connection"), Some("close"));
}
