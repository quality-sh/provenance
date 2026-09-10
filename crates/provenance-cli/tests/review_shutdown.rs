#![cfg(unix)]

use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

struct Host {
    child: Child,
    address: String,
    token: String,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Host {
    fn start(repo: &std::path::Path) -> Self {
        let mut child = Command::new(assert_cmd::cargo::cargo_bin("provenance"))
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
        let mut host = Self {
            child,
            address: String::new(),
            token: String::new(),
        };
        let config: Value =
            serde_json::from_str(&receive.recv_timeout(Duration::from_secs(15)).unwrap()).unwrap();
        host.address = config["endpoint"]
            .as_str()
            .unwrap()
            .strip_prefix("http://")
            .unwrap()
            .into();
        host.token = config["bearer"].as_str().unwrap().into();
        host
    }

    fn signal(&self, signal: &str) {
        assert!(Command::new("kill")
            .args([signal, &self.child.id().to_string()])
            .status()
            .unwrap()
            .success());
    }

    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "host did not exit after the second signal or completed write"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

fn repository() -> (tempfile::TempDir, ProvenanceLayout, Vec<u8>) {
    let repo = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(repo.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest = serde_json::to_vec(&Manifest::default_with_scope(
        ScopeId::new("default").unwrap(),
        RepoPathPrefix::new("."),
    ))
    .unwrap();
    std::fs::write(layout.manifest_path(), &manifest).unwrap();
    (repo, layout, manifest)
}

// Hold a real filesystem read inside an admitted write. A nonblocking FIFO
// writer opens only after the host opens the reader, so no timing guess is needed.
fn block_write(host: &Host, layout: &ProvenanceLayout) -> (File, TcpStream) {
    std::fs::remove_file(layout.manifest_path()).unwrap();
    assert!(Command::new("mkfifo")
        .arg(layout.manifest_path())
        .status()
        .unwrap()
        .success());
    let body = json!({"context":{"repository":"A","scope":"default"},"request":{
        "scope_id":"default", "parent":{"node_type":"requirement","node_id":"req_example"},
        "role":"user", "body":"The active write finishes before exit."
    }})
    .to_string();
    let mut stream = TcpStream::connect(&host.address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(stream, "POST /v8/operations/post-thread-message HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", host.address, host.token, body.len()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match rustix::fs::open(
            layout.manifest_path().as_std_path(),
            rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        ) {
            Ok(fd) => return (File::from(fd), stream),
            Err(rustix::io::Errno::NXIO) => {}
            Err(error) => panic!("FIFO open failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "operation did not start its filesystem read"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn first_signal_waits_for_an_active_write_to_finish() {
    let (repo, layout, manifest) = repository();
    let mut host = Host::start(repo.path());
    let (mut blocked, mut request) = block_write(&host, &layout);
    host.signal("-TERM");
    std::thread::sleep(Duration::from_millis(1200));
    assert!(
        host.child.try_wait().unwrap().is_none(),
        "active writes have no drain deadline"
    );
    // Restore the normal file before releasing the read, for later store access.
    std::fs::remove_file(layout.manifest_path()).unwrap();
    std::fs::write(layout.manifest_path(), &manifest).unwrap();
    blocked.write_all(&manifest).unwrap();
    drop(blocked);
    let mut response = String::new();
    request.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(host.wait().success());
    let store = StateStore::new(layout);
    let scope = ScopeId::new("default").unwrap();
    let threads = store.list_threads(&scope).unwrap();
    assert_eq!(threads.len(), 1);
    let messages = store.list_messages(&scope).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].body, "The active write finishes before exit.");
    assert!(TcpListener::bind(&host.address).is_ok());
}

#[test]
fn second_signal_forces_exit_from_a_blocked_operation_with_a_warning() {
    for (first, second) in [("-TERM", "-TERM"), ("-INT", "-INT"), ("-TERM", "-INT")] {
        let (repo, layout, _) = repository();
        let mut host = Host::start(repo.path());
        let (_blocked, _request) = block_write(&host, &layout);
        host.signal(first);
        std::thread::sleep(Duration::from_millis(1200));
        assert!(host.child.try_wait().unwrap().is_none());
        host.signal(second);
        let status = host.wait();
        assert_eq!(status.code(), Some(1), "forced exit must report failure");
        let mut errors = String::new();
        host.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut errors)
            .unwrap();
        assert!(errors.contains("send a second signal"), "{errors}");
        assert!(errors.contains("writes may be incomplete"), "{errors}");
        assert!(TcpStream::connect(&host.address).is_err());
        assert!(TcpListener::bind(&host.address).is_ok());
    }
}
