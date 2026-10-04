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

impl Host {
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

    pub fn take_stderr(&mut self) -> std::process::ChildStderr {
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

pub fn start_capturing_stderr(root: &std::path::Path) -> Host {
    let mut command = review_command(root);
    command.stderr(Stdio::piped());
    start_command(command)
}

pub fn review_command(root: &std::path::Path) -> Command {
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
