use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

pub struct Host {
    child: Child,
    pub config: Value,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
impl Host {
    pub fn terminate_and_wait(&mut self, timeout: Duration) -> std::process::ExitStatus {
        assert!(Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status()
            .unwrap()
            .success());
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "host shutdown timed out"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

pub fn repository() -> tempfile::TempDir {
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

pub fn set_disposition_actors(root: &std::path::Path, actor_ids: &[&str]) {
    let layout = provenance_store::layout::ProvenanceLayout::new(root.to_str().unwrap());
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids = actor_ids.iter().map(|id| (*id).to_owned()).collect();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

pub fn start(root: &std::path::Path) -> Host {
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

pub fn request(host: &Host, method: &str, path: &str, auth: bool) -> ureq::Request {
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

pub fn response(result: Result<ureq::Response, ureq::Error>) -> ureq::Response {
    match result {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("{error}"),
    }
}

pub fn list_discussions(host: &Host) -> ureq::Response {
    response(request(host, "GET", "/discussion-containers", true).call())
}
