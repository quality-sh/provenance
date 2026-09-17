use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Cursor, Read, Write},
    net::TcpStream,
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
    for (path, expected) in ASSETS.iter().copied().chain(std::iter::once((
        "/",
        ASSETS
            .iter()
            .find(|(path, _)| *path == "/index.html")
            .unwrap()
            .1,
    ))) {
        for method in ["GET", "HEAD"] {
            let reply = request(endpoint, method, path, &[], &[]);
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
                    request(endpoint, method, path, &[(header, value)], &[]).status(),
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
            request(endpoint, "GET", path, &[], &[]).status(),
            404,
            "{path}"
        );
    }
    assert_eq!(
        request(endpoint, "GET", "/review-config", &[], &[]).status(),
        401
    );
    assert_eq!(
        request(endpoint, "GET", "/discussion-containers", &[], b"invalid").status(),
        401
    );
}

struct WireResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl WireResponse {
    fn status(&self) -> u16 {
        self.status
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    fn into_reader(self) -> Cursor<Vec<u8>> {
        Cursor::new(self.body)
    }
}

fn request(
    endpoint: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> WireResponse {
    let authority = endpoint.strip_prefix("http://").unwrap();
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    write!(stream, "{method} {path} HTTP/1.1\r\n").unwrap();
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Host"))
    {
        write!(stream, "Host: {authority}\r\n").unwrap();
    }
    write!(
        stream,
        "Connection: close\r\nContent-Length: {}\r\n",
        body.len()
    )
    .unwrap();
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n").unwrap();
    }
    stream.write_all(b"\r\n").unwrap();
    stream.write_all(body).unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    let head = std::str::from_utf8(&response[..header_end]).unwrap();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .map(|line| {
            let (name, value) = line.split_once(':').unwrap();
            (name.to_ascii_lowercase(), value.trim().to_owned())
        })
        .collect();
    WireResponse {
        status,
        headers,
        body: response[(header_end + 4)..].to_vec(),
    }
}
