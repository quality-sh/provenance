//! Public CLI stream fixture with bounded waits and child cleanup.
use super::{waits, Host};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Stdio},
    sync::mpsc,
    time::Duration,
};

pub struct Follow {
    pub child: Child,
    lines: mpsc::Receiver<String>,
}
impl Follow {
    pub fn start(host: &Host, root: &Path, id: &str) -> Self {
        Self::with_options(host, root, id, &[])
    }
    pub fn with_options(host: &Host, root: &Path, id: &str, options: &[&str]) -> Self {
        let mut command = host.cli();
        command
            .args([
                id,
                "get",
                "--view",
                "review",
                "--follow",
                "--format",
                "json",
                "--repo",
                root.to_str().unwrap(),
            ])
            .args(options);
        Self::spawn(command)
    }
    pub fn spawn(mut command: std::process::Command) -> Self {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if send.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        Self { child, lines }
    }
    pub fn frame(&self) -> Value {
        serde_json::from_str(
            &self
                .lines
                .recv_timeout(Duration::from_secs(15))
                .expect("follow must flush a state frame"),
        )
        .unwrap()
    }
    pub fn until(&self, text: &str) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            let line = self
                .lines
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .expect("saved change must reach follow");
            if line.contains(text) {
                return serde_json::from_str(&line).unwrap();
            }
        }
    }
}
impl Drop for Follow {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub fn create(root: &Path, id: &str, kind: &str, extra: &[&str]) {
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    cmd.args([
        id,
        "create",
        "--type",
        kind,
        "--statement",
        "The agent receives saved changes.",
        "--repo",
        root.to_str().unwrap(),
    ])
    .args(extra);
    let result = waits::output(&mut cmd);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
