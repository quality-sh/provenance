#![cfg(unix)]

#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_core::ScopeId;
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
use review_host_support::{repository as initialized_repository, start_capturing_stderr, Host};
use serde_json::json;
use std::{
    fs::File,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    time::{Duration, Instant},
};

fn repository() -> (tempfile::TempDir, ProvenanceLayout, Vec<u8>) {
    let repo = initialized_repository();
    let layout = ProvenanceLayout::new(repo.path().to_str().unwrap());
    let scope = ScopeId::new("default").unwrap();
    StateStore::new(layout.clone())
        .create_requirement(provenance_store::state_store::CreateRequirementInput {
            scope_id: scope.clone(),
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
    let requirements = provenance_store::shards::requirements_path(&layout, &scope);
    (repo, layout, std::fs::read(requirements).unwrap())
}

// Hold a real filesystem read inside an admitted write. A nonblocking FIFO
// writer opens only after the host opens the reader, so no timing guess is needed.
fn block_write(host: &Host, layout: &ProvenanceLayout) -> (File, TcpStream) {
    let path =
        provenance_store::shards::requirements_path(layout, &ScopeId::new("default").unwrap());
    std::fs::remove_file(&path).unwrap();
    assert!(Command::new("mkfifo")
        .arg(&path)
        .status()
        .unwrap()
        .success());
    let body = json!({"data":{
        "actor":"ben", "role":"user", "body":"The active write finishes before exit."
    }})
    .to_string();
    let mut stream = TcpStream::connect(host.address()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(stream, "POST /requirements/req_example/discussions HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", host.address(), host.bearer(), body.len()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match rustix::fs::open(
            path.as_std_path(),
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
/// Implementation aid: pins graceful shutdown ordering around an admitted write.
fn first_signal_waits_for_an_active_write_to_finish() {
    let (repo, layout, requirements) = repository();
    let mut host = start_capturing_stderr(repo.path());
    let (mut blocked, mut request) = block_write(&host, &layout);
    host.signal("-TERM");
    std::thread::sleep(Duration::from_millis(1200));
    assert!(host.is_running(), "active writes have no drain deadline");
    // Restore the normal file before releasing the read, for later store access.
    let path =
        provenance_store::shards::requirements_path(&layout, &ScopeId::new("default").unwrap());
    std::fs::remove_file(&path).unwrap();
    std::fs::write(path, &requirements).unwrap();
    blocked.write_all(&requirements).unwrap();
    drop(blocked);
    let mut response = String::new();
    request.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(host.wait(Duration::from_secs(5)).success());
    let store = StateStore::new(layout);
    let scope = ScopeId::new("default").unwrap();
    let threads = store.list_threads(&scope).unwrap();
    assert_eq!(threads.len(), 1);
    let messages = store.list_messages(&scope).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].body, "The active write finishes before exit.");
    assert!(TcpListener::bind(host.address()).is_ok());
}

#[test]
/// Implementation aid: pins forced shutdown and its incomplete-write warning.
fn second_signal_forces_exit_from_a_blocked_operation_with_a_warning() {
    for (first, second) in [("-TERM", "-TERM"), ("-INT", "-INT"), ("-TERM", "-INT")] {
        let (repo, layout, _) = repository();
        let mut host = start_capturing_stderr(repo.path());
        let (_blocked, _request) = block_write(&host, &layout);
        host.signal(first);
        std::thread::sleep(Duration::from_millis(1200));
        assert!(host.is_running());
        host.signal(second);
        let status = host.wait(Duration::from_secs(5));
        assert_eq!(status.code(), Some(1), "forced exit must report failure");
        let mut errors = String::new();
        host.take_stderr().read_to_string(&mut errors).unwrap();
        assert!(errors.contains("send a second signal"), "{errors}");
        assert!(errors.contains("writes may be incomplete"), "{errors}");
        assert!(TcpStream::connect(host.address()).is_err());
        assert!(TcpListener::bind(host.address()).is_ok());
    }
}
