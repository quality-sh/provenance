#![cfg(unix)]

use provenance_macros::verifies;
use provenance_transport::local_host::{discover, LocalHostRegistration};
use serde_json::{json, Value};
use std::{
    fs::Permissions,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

struct ProbeHost {
    endpoint: String,
    response: Arc<Mutex<Value>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ProbeHost {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let response = Arc::new(Mutex::new(json!({})));
        let thread_response = response.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let mut request = [0_u8; 2048];
                let count = stream.read(&mut request).unwrap_or(0);
                let path_matches = String::from_utf8_lossy(&request[..count])
                    .starts_with("GET /local-host-identity HTTP/1.1");
                let (status, body) = if path_matches {
                    ("200 OK", thread_response.lock().unwrap().to_string())
                } else {
                    ("404 Not Found", "{}".to_owned())
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        Self {
            endpoint,
            response,
            stop,
            thread: Some(thread),
        }
    }

    fn answer_with(&self, identity: impl serde::Serialize) {
        *self.response.lock().unwrap() = serde_json::to_value(identity).unwrap();
    }
}

impl Drop for ProbeHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.endpoint.trim_start_matches("http://"));
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn registry(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".provenance/cache/local-hosts/default.json")
}

#[test]
fn registration_publishes_its_identity() {
    let repository = tempfile::tempdir().unwrap();
    let registration = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1234",
        "local",
    )
    .unwrap();
    let path = registry(repository.path());
    let published: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();

    assert_eq!(published["schemaVersion"], 1);
    assert_eq!(published["endpoint"], "http://127.0.0.1:1234");
    assert_eq!(published["repositoryId"], "local");
    assert_eq!(published["scope"], "default");
    assert_eq!(
        published["instanceNonce"],
        registration.identity().instance_nonce
    );
}

#[test]
fn registration_does_not_publish_credentials() {
    let repository = tempfile::tempdir().unwrap();
    let _registration = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1234",
        "local",
    )
    .unwrap();
    let published: Value =
        serde_json::from_slice(&std::fs::read(registry(repository.path())).unwrap()).unwrap();

    assert!(published.get("bearer").is_none());
}

#[test]
fn registration_uses_owner_only_permissions() {
    let repository = tempfile::tempdir().unwrap();
    let _registration = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1234",
        "local",
    )
    .unwrap();
    let path = registry(repository.path());

    assert_eq!(
        std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o077,
        0
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
        0
    );
}

#[test]
fn registration_removes_its_record_on_drop() {
    let repository = tempfile::tempdir().unwrap();
    let registration = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1234",
        "local",
    )
    .unwrap();
    let path = registry(repository.path());

    drop(registration);

    assert!(!path.exists());
}

#[test]
fn publication_replaces_permissive_modes() {
    let repository = tempfile::tempdir().unwrap();
    let path = registry(repository.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::set_permissions(path.parent().unwrap(), Permissions::from_mode(0o777)).unwrap();
    std::fs::write(&path, b"{}").unwrap();
    std::fs::set_permissions(&path, Permissions::from_mode(0o666)).unwrap();

    let registration = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1234",
        "local",
    )
    .unwrap();

    assert_eq!(
        std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o077,
        0
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
        0
    );
    drop(registration);
}

#[test]
fn publication_refuses_a_live_slot_owner() {
    let repository = tempfile::tempdir().unwrap();
    let first_host = ProbeHost::start();
    let first =
        LocalHostRegistration::publish(repository.path(), "default", &first_host.endpoint, "local")
            .unwrap();
    first_host.answer_with(first.identity());

    let error = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1235",
        "local",
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("already has a running local host"));
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(registry(repository.path())).unwrap())
            .unwrap()["instanceNonce"],
        first.identity().instance_nonce
    );
}

#[test]
fn publication_reclaims_a_stale_slot() {
    let repository = tempfile::tempdir().unwrap();
    let stale =
        LocalHostRegistration::publish(repository.path(), "default", "http://127.0.0.1:1", "old")
            .unwrap();
    std::mem::forget(stale);

    let replacement = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1234",
        "new",
    )
    .unwrap();
    let published: Value =
        serde_json::from_slice(&std::fs::read(registry(repository.path())).unwrap()).unwrap();

    assert_eq!(published["repositoryId"], "new");
    assert_eq!(
        published["instanceNonce"],
        replacement.identity().instance_nonce
    );
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn discovery_returns_a_matching_live_host() {
    let repository = tempfile::tempdir().unwrap();
    let host = ProbeHost::start();
    let registration =
        LocalHostRegistration::publish(repository.path(), "default", &host.endpoint, "local")
            .unwrap();

    host.answer_with(registration.identity());
    let discovered = discover(repository.path(), "default").unwrap().unwrap();
    assert_eq!(
        discovered.endpoint().as_str(),
        format!("{}/", host.endpoint)
    );
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn discovery_rejects_an_identity_mismatch() {
    let repository = tempfile::tempdir().unwrap();
    let host = ProbeHost::start();
    let registration =
        LocalHostRegistration::publish(repository.path(), "default", &host.endpoint, "local")
            .unwrap();

    host.answer_with(json!({
        "schemaVersion": 1,
        "repositoryId": "other",
        "scope": "default",
        "instanceNonce": registration.identity().instance_nonce,
    }));
    assert!(discover(repository.path(), "default").unwrap().is_none());
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn discovery_removes_stale_state() {
    let repository = tempfile::tempdir().unwrap();
    let registration =
        LocalHostRegistration::publish(repository.path(), "default", "http://127.0.0.1:1", "local")
            .unwrap();
    std::mem::forget(registration);

    assert!(discover(repository.path(), "default").unwrap().is_none());
    assert!(!registry(repository.path()).exists());
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn publication_rejects_non_loopback_or_non_base_endpoints() {
    let repository = tempfile::tempdir().unwrap();
    for endpoint in [
        "https://127.0.0.1:1234",
        "http://localhost:1234",
        "http://127.0.0.1:1234/path",
        "http://user@127.0.0.1:1234",
        "http://127.0.0.1",
    ] {
        assert!(
            LocalHostRegistration::publish(repository.path(), "default", endpoint, "local")
                .is_err()
        );
    }
}

#[test]
fn published_registry_matches_the_version_one_fixture() {
    let expected_registry: Value = serde_json::from_str(include_str!(
        "../../../docs/fixtures/local-host/registry-v1.json"
    ))
    .unwrap();
    let repository = tempfile::tempdir().unwrap();
    let registration = LocalHostRegistration::publish(
        repository.path(),
        expected_registry["scope"].as_str().unwrap(),
        expected_registry["endpoint"].as_str().unwrap(),
        expected_registry["repositoryId"].as_str().unwrap(),
    )
    .unwrap();
    let mut published: Value =
        serde_json::from_slice(&std::fs::read(registry(repository.path())).unwrap()).unwrap();
    published["instanceNonce"] = expected_registry["instanceNonce"].clone();

    assert_eq!(published, expected_registry);
}

#[test]
fn published_identity_matches_the_version_one_fixture() {
    let expected_identity: Value = serde_json::from_str(include_str!(
        "../../../docs/fixtures/local-host/identity-v1.json"
    ))
    .unwrap();
    let repository = tempfile::tempdir().unwrap();
    let registration = LocalHostRegistration::publish(
        repository.path(),
        expected_identity["scope"].as_str().unwrap(),
        "http://127.0.0.1:41731",
        expected_identity["repositoryId"].as_str().unwrap(),
    )
    .unwrap();
    let mut identity = serde_json::to_value(registration.identity()).unwrap();
    identity["instanceNonce"] = expected_identity["instanceNonce"].clone();

    assert_eq!(identity, expected_identity);
}
