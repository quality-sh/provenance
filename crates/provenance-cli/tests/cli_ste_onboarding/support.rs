//! Shared pieces of the STE onboarding tests: a loopback asset server, a
//! fixture with its own STE caches, and the init arguments.

use assert_cmd::Command;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

/// These tests each spawn a debug-build `provenance` doing CPU-heavy PDF
/// imports against a 10s-timeout HTTP client. Run concurrently on a small CI
/// runner they starve each other into client timeouts and silent retries,
/// which the request-count assertions then observe (the onboarding tests have failed
/// exactly that way on main). Serialize them.
static SERIAL: Mutex<()> = Mutex::new(());

pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub struct Fixture {
    pub temporary: tempfile::TempDir,
    pub repo: PathBuf,
    pub asset_dir: PathBuf,
    pub index_dir: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        Self {
            repo: temporary.path().join("repo"),
            asset_dir: temporary.path().join("assets"),
            index_dir: temporary.path().join("indexes"),
            temporary,
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
        command
            .env("PROVENANCE_STE100_ASSET_DIR", &self.asset_dir)
            .env("PROVENANCE_STE100_INDEX_DIR", &self.index_dir);
        command
    }

    pub fn init(&self) -> Command {
        let mut command = self.command();
        command.args(init_args(&self.repo));
        command
    }
}

pub fn init_args(repo: &Path) -> Vec<String> {
    [
        "init",
        "--path",
        repo.to_str().unwrap(),
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub struct TestServer {
    pub address: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    hold: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    pub fn new(status: u16, body: &[u8]) -> Self {
        Self::start(status, body, false)
    }

    /// Records each request at once, but sends no response until `release`.
    pub fn held(status: u16, body: &[u8]) -> Self {
        Self::start(status, body, true)
    }

    fn start(status: u16, body: &[u8], hold: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let shared_requests = Arc::clone(&requests);
        let shared_stop = Arc::clone(&stop);
        let hold = Arc::new(AtomicBool::new(hold));
        let shared_hold = Arc::clone(&hold);
        let body = body.to_vec();
        let thread = thread::spawn(move || {
            while !shared_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    // Drop's wake-up socket can land here before the stop flag
                    // is observed; never serve once shutdown has begun.
                    Ok(_) if shared_stop.load(Ordering::SeqCst) => break,
                    Ok((stream, _)) => {
                        serve(stream, status, &body, &shared_requests, &shared_hold);
                    }
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
            hold,
            thread: Some(thread),
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}/ASD-STE100_ISSUE9.pdf", self.address)
    }

    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    pub fn release(&self) {
        self.hold.store(false, Ordering::SeqCst);
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.release();
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn serve(
    mut stream: TcpStream,
    status: u16,
    body: &[u8],
    requests: &Mutex<Vec<String>>,
    hold: &AtomicBool,
) {
    // BSD-derived systems (macOS) make accepted sockets inherit the
    // listener's O_NONBLOCK; a nonblocking stream turns write_all into
    // silent truncation (or a panic, before the hardening) as soon as the
    // client is slow to drain the body, which the client reports as
    // "response body closed before all bytes were read" and retries.
    stream.set_nonblocking(false).unwrap();
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
    while hold.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(10));
    }
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
