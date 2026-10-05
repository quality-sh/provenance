#![allow(dead_code, clippy::duplicated_attributes)]

use provenance_transport::local_host::LAUNCH_SESSION_ROUTE;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

pub struct Host {
    child: Child,
    pub config: Value,
    user_cache: tempfile::TempDir,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Host {
    /// Runs the CLI as the same user as the host, with the same per-user cache.
    pub fn cli(&self) -> Command {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
        use_user_cache(&mut command, self.user_cache.path());
        command
    }

    /// Returns the launch key file that the host wrote for its instance.
    pub fn launch_key_path(&self) -> PathBuf {
        cache_directory(self.user_cache.path())
            .join("provenance/review-launch")
            .join(format!(
                "{}.key",
                self.config["instanceNonce"].as_str().unwrap()
            ))
    }

    pub fn address(&self) -> &str {
        self.config["endpoint"]
            .as_str()
            .unwrap()
            .strip_prefix("http://")
            .unwrap()
    }

    pub fn bearer(&self) -> &str {
        self.config["bearer"].as_str().unwrap()
    }

    #[cfg(unix)]
    pub fn signal(&self, signal: &str) {
        assert!(Command::new("kill")
            .args([signal, &self.child.id().to_string()])
            .status()
            .unwrap()
            .success());
    }

    pub fn wait(&mut self, timeout: Duration) -> std::process::ExitStatus {
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

    pub const fn take_stderr(&mut self) -> std::process::ChildStderr {
        self.child.stderr.take().expect("piped host stderr")
    }

    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }

    pub fn terminate_and_wait(&mut self, timeout: Duration) -> std::process::ExitStatus {
        #[cfg(unix)]
        self.signal("-TERM");
        #[cfg(not(unix))]
        self.child.kill().unwrap();
        self.wait(timeout)
    }

    pub fn stop_gracefully(&mut self, timeout: Duration) {
        let status = self.terminate_and_wait(timeout);
        #[cfg(unix)]
        assert!(status.success(), "review host did not exit cleanly");
        #[cfg(not(unix))]
        {
            let _ = status;
        }
    }
}

pub fn repository() -> tempfile::TempDir {
    repository_with_actors(&[])
}

pub fn repository_with_actors(actor_ids: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    initialize_repository(&mut command, dir.path(), actor_ids);
    dir
}

pub fn initialize_repository(command: &mut Command, root: &std::path::Path, actor_ids: &[&str]) {
    command.args([
        "init",
        "--path",
        root.to_str().unwrap(),
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    set_actor_arguments(command, actor_ids);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn configure_disposition_actors(root: &std::path::Path, actor_ids: &[&str]) {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.args(["init", "--path", root.to_str().unwrap()]);
    set_actor_arguments(&mut command, actor_ids);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn set_actor_arguments(command: &mut Command, actor_ids: &[&str]) {
    if actor_ids.is_empty() {
        command.arg("--clear-disposition-actors");
    } else {
        for actor_id in actor_ids {
            command.args(["--disposition-actor-id", actor_id]);
        }
    }
}

pub fn start(root: &std::path::Path) -> Host {
    let mut command = review_command(root);
    command.stderr(Stdio::inherit());
    start_command(command)
}

/// Starts a host that may open the browser; the caller sets up the command first.
pub fn start_with(root: &std::path::Path, configure: impl FnOnce(&mut Command)) -> Host {
    let mut command = host_command(root);
    command.stderr(Stdio::inherit());
    configure(&mut command);
    start_command(command)
}

pub fn start_capturing_stderr(root: &std::path::Path) -> Host {
    let mut command = review_command(root);
    command.stderr(Stdio::piped());
    start_command(command)
}

/// Builds a host command that never opens a browser.
pub fn review_command(root: &std::path::Path) -> Command {
    let mut command = host_command(root);
    command.arg("--no-open");
    command
}

fn host_command(root: &std::path::Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.args([
        "review",
        "--repo",
        root.to_str().unwrap(),
        "--repository-id",
        "A",
        "--scope",
        "default",
    ]);
    command
}

pub fn start_command(mut command: Command) -> Host {
    let user_cache = tempfile::tempdir().unwrap();
    use_user_cache(&mut command, user_cache.path());
    let mut child = command.stdout(Stdio::piped()).spawn().unwrap();
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
        user_cache,
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

/// Points every platform's per-user cache at one isolated directory.
fn use_user_cache(command: &mut Command, root: &Path) {
    command
        .env("HOME", root)
        .env("XDG_CACHE_HOME", cache_directory(root))
        .env("LOCALAPPDATA", cache_directory(root));
}

fn cache_directory(root: &Path) -> PathBuf {
    root.join("Library").join("Caches")
}

/// A BROWSER program that records each page it gets: sh on Unix, a command script on Windows.
pub struct BrowserRecorder {
    directory: tempfile::TempDir,
}

impl BrowserRecorder {
    pub fn install() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let record = directory.path().join("opened.html");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let script = format!("#!/bin/sh\ncat \"$1\" >> '{}'\n", record.display());
            std::fs::write(Self::program_in(directory.path()), script).unwrap();
            std::fs::set_permissions(
                Self::program_in(directory.path()),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap();
        }
        #[cfg(windows)]
        std::fs::write(
            Self::program_in(directory.path()),
            format!("@type \"%~1\" >> \"{}\"\r\n", record.display()),
        )
        .unwrap();
        Self { directory }
    }

    fn program_in(directory: &Path) -> PathBuf {
        directory.join(if cfg!(windows) {
            "browser.cmd"
        } else {
            "browser.sh"
        })
    }

    /// Makes the command a local desktop session whose browser is this recorder.
    pub fn configure(&self, command: &mut Command) {
        command
            .env("BROWSER", Self::program_in(self.directory.path()))
            .env("DISPLAY", ":0")
            .env_remove("SSH_CONNECTION")
            .env_remove("SSH_CLIENT")
            .env_remove("SSH_TTY");
    }

    /// Returns the link of the first page the browser got, if it got one.
    pub fn opened_link(&self) -> Option<String> {
        let page = std::fs::read_to_string(self.directory.path().join("opened.html")).ok()?;
        let start = page.find("href=\"")? + "href=\"".len();
        let end = start + page[start..].find('"')?;
        Some(page[start..end].replace("&amp;", "&"))
    }

    pub fn wait_for_link(&self, timeout: Duration) -> String {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(link) = self.opened_link() {
                return link;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the browser did not get a page"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

/// Takes the launch code from the fragment of a review link.
pub fn launch_code(link: &str) -> String {
    let fragment = url::Url::parse(link)
        .unwrap()
        .fragment()
        .unwrap()
        .to_owned();
    fragment.strip_prefix("launch=").unwrap().to_owned()
}

/// Exchanges a launch code for the page session as the review page does.
pub fn redeem(host: &Host, code: &str) -> ureq::Response {
    response(
        request(host, "POST", LAUNCH_SESSION_ROUTE, false)
            .set("Content-Type", "application/json")
            .send_string(&json!({ "code": code }).to_string()),
    )
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
