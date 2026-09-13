use std::{
    io::{BufRead, BufReader, Read},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

use provenance_macros::verifies;

include!(concat!(env!("OUT_DIR"), "/review_assets.rs"));

struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(repo: &std::path::Path) -> (Host, serde_json::Value) {
    let mut host = Host(
        Command::new(assert_cmd::cargo::cargo_bin("provenance"))
            .args([
                "review",
                "--repo",
                repo.to_str().unwrap(),
                "--repository-id",
                "A",
                "--scope",
                "default",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let stdout = host.0.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        let _ = send.send(line);
    });
    let config: serde_json::Value =
        serde_json::from_str(&receive.recv_timeout(Duration::from_secs(15)).unwrap()).unwrap();
    (host, config)
}

#[test]
// The loop tries every compiled asset over GET and HEAD, byte for byte.
#[verifies("rule_cli_serves_review_assets", exhaustion)]
fn accepted_inventory_uses_the_composed_router_and_origin_checks() {
    let repo = tempfile::tempdir().unwrap();
    let layout = provenance_store::layout::ProvenanceLayout::new(repo.path().to_str().unwrap());
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
    let (_host, config) = start(repo.path());
    let endpoint = config["endpoint"].as_str().unwrap();
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    for (path, expected) in ASSETS.iter().copied().chain(std::iter::once((
        "/",
        ASSETS
            .iter()
            .find(|(path, _)| *path == "/index.html")
            .unwrap()
            .1,
    ))) {
        for method in ["GET", "HEAD"] {
            let url = format!("{endpoint}{path}");
            let reply = response(agent.request(method, &url).call());
            assert_eq!(reply.status(), 200, "{method} {path}");
            assert_eq!(
                reply.header("Content-Length").unwrap(),
                expected.len().to_string()
            );
            assert_eq!(reply.header("Cache-Control"), Some("no-store"));
            assert_eq!(reply.header("X-Content-Type-Options"), Some("nosniff"));
            let mut bytes = Vec::new();
            reply.into_reader().read_to_end(&mut bytes).unwrap();
            assert_eq!(
                bytes,
                if method == "HEAD" { &[] } else { expected },
                "{method} {path}"
            );
            for (header, value) in [
                ("Origin", "https://unrelated.test"),
                ("Host", "unrelated.test"),
                ("Sec-Fetch-Site", "cross-site"),
            ] {
                assert_eq!(
                    response(agent.request(method, &url).set(header, value).call()).status(),
                    403,
                    "{method} {path} {header}"
                );
            }
        }
    }
    for path in [
        "/%2e%2e/Cargo.toml",
        "/.provenance/manifest.json",
        "/assets/missing.js",
    ] {
        assert_eq!(
            response(agent.get(&format!("{endpoint}{path}")).call()).status(),
            404,
            "{path}"
        );
    }
    assert_eq!(
        response(agent.get(&format!("{endpoint}/review-config")).call()).status(),
        401
    );
    assert_eq!(
        response(
            agent
                .post(&format!("{endpoint}/v9/operations/list-threads"))
                .send_string("invalid")
        )
        .status(),
        401
    );
}

fn response(result: Result<ureq::Response, ureq::Error>) -> ureq::Response {
    match result {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("{error}"),
    }
}
