use assert_cmd::Command;
use provenance_transport::local_host::{LocalHostIdentity, LocalHostRegistration, IDENTITY_ROUTE};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

pub fn initialized_repo() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    (directory, repo)
}

pub fn json_output(arguments: &[&str]) -> Value {
    let output = provenance().args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[allow(dead_code)]
pub fn json_stdin_output(arguments: &[&str], input: &Value) -> Value {
    let output = provenance()
        .args(arguments)
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub struct LocalHost {
    pub endpoint: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    _registration: LocalHostRegistration,
}

impl Drop for LocalHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.endpoint.trim_start_matches("http://"));
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

pub fn local_host(repo: &str) -> LocalHost {
    local_host_with_identity(repo, |identity| serde_json::to_value(identity).unwrap())
}

pub fn local_host_with_identity(
    repo: &str,
    response: impl FnOnce(LocalHostIdentity) -> Value,
) -> LocalHost {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let registration =
        LocalHostRegistration::publish(std::path::Path::new(repo), "default", &endpoint, "local")
            .unwrap();
    let body = response(registration.identity()).to_string();
    listener.set_nonblocking(true).unwrap();
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
                .starts_with(&format!("GET {IDENTITY_ROUTE} HTTP/1.1"));
            let (status, response) = if path_matches {
                ("200 OK", body.as_str())
            } else {
                ("404 Not Found", "{}")
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        }
    });
    LocalHost {
        endpoint,
        stop,
        thread: Some(thread),
        _registration: registration,
    }
}

pub fn write_local_host_fixture(repo: &str, endpoint: &str) {
    let mut fixture: Value = serde_json::from_str(include_str!(
        "../../../../docs/fixtures/local-host/registry-v1.json"
    ))
    .unwrap();
    fixture["endpoint"] = endpoint.into();
    let path = std::path::Path::new(repo).join(".provenance/cache/local-hosts/default.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        path.parent().unwrap(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    std::fs::write(&path, serde_json::to_vec(&fixture).unwrap()).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
