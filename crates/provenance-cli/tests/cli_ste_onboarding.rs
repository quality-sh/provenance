use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

#[path = "cli_dictionary/support.rs"]
#[allow(dead_code)]
mod dictionary_support;

/// These tests each spawn a debug-build `provenance` doing CPU-heavy PDF
/// imports against a 10s-timeout HTTP client. Run concurrently on a small CI
/// runner they starve each other into client timeouts and silent retries,
/// which the request-count assertions then observe (this test file has failed
/// exactly that way on main). Serialize them.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

const REQUEST_FORM: &str = "https://www.asd-ste100.org/STE_downloads.html#article02-2l";
const CHANGE_FORM: &str = "https://www.asd-ste100.org/STE_downloads.html#features038-31";

#[test]
#[verifies("rule_ste_dictionary_interactive_acquisition", examples)]
#[verifies("rule_ste_dictionary_attribution", examples)]
#[verifies("rule_ste_dictionary_claim_scope", examples)]
fn interactive_onboarding_imports_the_selected_pdf() {
    let _serial = serial();
    let fixture = Fixture::new();
    let pdf = fixture.temporary.path().join("issue-9.pdf");
    std::fs::write(&pdf, dictionary_support::dictionary_pdf()).unwrap();

    fixture
        .init("interactive")
        .args(["--ste-pdf", pdf.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(REQUEST_FORM))
        .stdout(predicate::str::contains(CHANGE_FORM))
        .stdout(predicate::str::contains("ASD owns ASD-STE100"))
        .stdout(predicate::str::contains("does not claim compliance"));

    assert!(dictionary_support::reference_path(&fixture.repo).is_file());
}

#[test]
#[verifies("rule_ste_dictionary_interactive_acquisition", examples)]
fn interactive_onboarding_directs_the_user_to_the_official_form() {
    let _serial = serial();
    let fixture = Fixture::new();

    fixture
        .init("interactive")
        .assert()
        .success()
        .stdout(predicate::str::contains(REQUEST_FORM))
        .stdout(predicate::str::contains("--ste-pdf"));

    assert!(!dictionary_support::reference_path(&fixture.repo).exists());
}

#[test]
#[verifies("rule_ste_dictionary_agent_acquisition", examples)]
#[verifies("rule_ste_dictionary_download_identity", examples)]
fn agent_onboarding_downloads_and_imports_the_official_asset() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_support::dictionary_pdf());
    let fixture = Fixture::new();

    fixture
        .init("agent")
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .assert()
        .success();

    assert!(dictionary_support::reference_path(&fixture.repo).is_file());
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].to_ascii_lowercase().contains(&format!(
        "user-agent: provenance/{}",
        env!("CARGO_PKG_VERSION")
    )));
}

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
#[verifies("rule_ste_dictionary_no_operational_download", examples)]
fn repeated_setup_and_normal_checks_use_local_data_without_network_access() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_support::dictionary_pdf());
    let fixture = Fixture::new();
    fixture
        .init("agent")
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .assert()
        .success();

    fixture
        .init("agent")
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .assert()
        .success();
    fixture
        .command()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .args(["check", "--repo", fixture.repo.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(server.requests().len(), 1);
}

#[test]
#[verifies("rule_ste_dictionary_download_concurrency", examples)]
fn concurrent_agent_onboarding_shares_one_download() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_support::dictionary_pdf());
    let temporary = tempfile::tempdir().unwrap();
    let asset_dir = temporary.path().join("assets");
    let index_dir = temporary.path().join("indexes");
    let mut children = Vec::new();

    for name in ["one", "two"] {
        let repo = temporary.path().join(name);
        let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
        children.push(
            command
                .env("PROVENANCE_STE100_ASSET_DIR", &asset_dir)
                .env("PROVENANCE_STE100_INDEX_DIR", &index_dir)
                .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
                .args(init_args(&repo, "agent"))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }

    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(server.requests().len(), 1);
}

#[test]
#[verifies("rule_ste_dictionary_download_retry_bound", examples)]
#[verifies("rule_ste_dictionary_asset_fallback", examples)]
fn exhausted_download_retries_fall_back_to_the_official_request_form() {
    let _serial = serial();
    let server = TestServer::new(503, b"unavailable");
    let fixture = Fixture::new();

    fixture
        .init("agent")
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .assert()
        .success()
        .stdout(predicate::str::contains(REQUEST_FORM));

    assert_eq!(server.requests().len(), 3);
    assert!(!dictionary_support::reference_path(&fixture.repo).exists());
}

struct Fixture {
    temporary: tempfile::TempDir,
    repo: PathBuf,
    asset_dir: PathBuf,
    index_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        Self {
            repo: temporary.path().join("repo"),
            asset_dir: temporary.path().join("assets"),
            index_dir: temporary.path().join("indexes"),
            temporary,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
        command
            .env("PROVENANCE_STE100_ASSET_DIR", &self.asset_dir)
            .env("PROVENANCE_STE100_INDEX_DIR", &self.index_dir);
        command
    }

    fn init(&self, mode: &str) -> Command {
        let mut command = self.command();
        command.args(init_args(&self.repo, mode));
        command
    }
}

fn init_args(repo: &Path, mode: &str) -> Vec<String> {
    [
        "init",
        "--path",
        repo.to_str().unwrap(),
        "--scope",
        "default",
        "--path-prefix",
        ".",
        "--ste-onboarding",
        mode,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn server_ignores_connections_that_send_nothing() {
    let _serial = serial();
    let server = TestServer::new(200, b"body");
    // Same shape as Drop's wake-up socket: connect, send nothing, hang up.
    drop(TcpStream::connect(server.address).unwrap());
    thread::sleep(Duration::from_millis(50));
    assert_eq!(server.requests().len(), 0);
}

struct TestServer {
    address: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn new(status: u16, body: &[u8]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let shared_requests = Arc::clone(&requests);
        let shared_stop = Arc::clone(&stop);
        let body = body.to_vec();
        let thread = thread::spawn(move || {
            while !shared_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    // Drop's wake-up socket can land here before the stop flag
                    // is observed; never serve once shutdown has begun.
                    Ok(_) if shared_stop.load(Ordering::SeqCst) => break,
                    Ok((stream, _)) => serve(stream, status, &body, &shared_requests),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept test request: {error}"),
                }
            }
        });
        Self {
            address,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/ASD-STE100_ISSUE9.pdf", self.address)
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn serve(mut stream: TcpStream, status: u16, body: &[u8], requests: &Mutex<Vec<String>>) {
    // Generous: on loaded CI runners the client can sit descheduled for
    // seconds between connect and first byte; giving up early drops real
    // requests from the recorded count. The timeout only guards against a
    // connection that never progresses at all.
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !bytes.ends_with(b"\r\n\r\n") {
        // A torn or instantly-closed connection (Drop's wake-up socket shows
        // up as a reset on Windows) must never panic the accept thread.
        let read = match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        bytes.extend_from_slice(&buffer[..read]);
    }
    if bytes.is_empty() {
        return;
    }
    requests
        .lock()
        .unwrap()
        .push(String::from_utf8(bytes).unwrap());
    let reason = if status == 200 {
        "OK"
    } else {
        "Service Unavailable"
    };
    // Response failures surface as a failed download in the test under
    // observation; they must not panic the shared accept thread.
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
}
