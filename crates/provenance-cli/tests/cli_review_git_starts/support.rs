//! A logging `git` for the review host, and the requests of the Git start tests.

use super::review_host_support::{request, response, review_command, start_command, Host};
use serde_json::{json, Value};
use std::{
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

/// The Git starts of review host processes, read from the log of a `git` first on PATH.
pub struct GitLog {
    tools: tempfile::TempDir,
    log: PathBuf,
    global: PathBuf,
    read: usize,
}

impl GitLog {
    /// Writes the logging `git` and a per-user Git configuration with `git_config`.
    pub fn new(git_config: &str) -> Self {
        let tools = tempfile::tempdir().unwrap();
        let log = tools.path().join("git-starts.log");
        let global = tools.path().join("gitconfig");
        std::fs::write(&log, "").unwrap();
        std::fs::write(&global, git_config).unwrap();
        let real = std::process::Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap();
        let real = String::from_utf8(real.stdout).unwrap();
        let wrapper = tools.path().join("git");
        std::fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
                log.display(),
                real.trim()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            tools,
            log,
            global,
            read: 0,
        }
    }

    /// Replaces the per-user Git configuration.
    pub fn set_config(&self, git_config: &str) {
        std::fs::write(&self.global, git_config).unwrap();
    }

    /// Starts a review host of `repo` that runs the logging `git`.
    pub fn start_host(&self, repo: &Path) -> Host {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(self.tools.path().to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .unwrap();
        let mut command = review_command(repo);
        command
            .env("PATH", path)
            .env("GIT_CONFIG_GLOBAL", &self.global)
            .env("GIT_CONFIG_NOSYSTEM", "1");
        start_command(command)
    }

    /// The Git subcommands that started after the last call, in start order.
    pub fn step(&mut self) -> Vec<String> {
        let text = std::fs::read_to_string(&self.log).unwrap_or_else(|error| {
            panic!("the Git log {} is unreadable: {error}", self.log.display())
        });
        let lines = text.lines().collect::<Vec<_>>();
        let new = lines[self.read..]
            .iter()
            .map(|line| subcommand(line))
            .collect();
        self.read = lines.len();
        new
    }
}

/// The Git subcommand of one logged argument list, after the global options. A start
/// with only options, such as `--version`, gives its first option.
fn subcommand(line: &str) -> String {
    let mut words = line.split_whitespace();
    let first = words.clone().next().unwrap_or_default().to_owned();
    while let Some(word) = words.next() {
        if matches!(word, "-c" | "-C") {
            words.next();
        } else if !word.starts_with('-') {
            return word.to_owned();
        }
    }
    first
}

/// Commits all files of `repo` and returns the commit.
pub fn commit_all(repo: &Path, message: &str) -> String {
    if !repo.join(".git").exists() {
        git(repo, &["init", "-q"]);
    }
    git(repo, &["add", "-A"]);
    git(
        repo,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
    git(repo, &["rev-parse", "HEAD"])
}

/// Runs Git in `directory` from this test process, so the host log does not count it.
pub fn git(directory: &Path, args: &[&str]) -> String {
    let output = provenance_store::git_process::git()
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

pub fn get(host: &Host, path: &str) -> Value {
    let read = response(request(host, "GET", path, true).call());
    assert_eq!(read.status(), 200, "GET {path}");
    serde_json::from_str(&read.into_string().unwrap()).unwrap()
}

/// The `reviewer` of the review configuration.
pub fn reviewer(host: &Host) -> Value {
    get(host, "/review-config")["reviewer"].clone()
}

/// The configuration, the Requirement and its document, as the review page reads them.
/// Returns the reviewer.
pub fn page_load(host: &Host, id: &str) -> Value {
    let reviewer = reviewer(host);
    assert_eq!(get(host, &format!("/requirements/{id}"))["data"]["id"], id);
    let document = get(host, &format!("/requirements/{id}/document"));
    assert!(document.get("data").is_some(), "{document}");
    reviewer
}

/// The version ids and authors of the history of one Requirement, oldest first.
pub fn history(host: &Host, id: &str) -> Vec<(String, Option<String>)> {
    get(host, &format!("/requirements/{id}/history"))["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["id"].as_str().unwrap().to_owned(),
                item["author"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

pub fn accept(host: &Host, proposal: &str) {
    let status = response(
        request(
            host,
            "POST",
            &format!("/requirements/req_review/submissions/{proposal}/decide"),
            true,
        )
        .set("Content-Type", "application/json")
        .send_string(
            &json!({"data": {
                "actor": {"identity_type": "human", "id": "jane_example_com"},
                "decision": "accepted", "rationale": null,
                "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "req_review"},
                "feedback": null, "declared_by": null
            }})
            .to_string(),
        ),
    )
    .status();
    assert!((200..300).contains(&status), "decide: {status}");
}

pub fn comment(host: &Host) {
    let status = response(
        request(host, "POST", "/requirements/req_review/discussions", true)
            .set("Content-Type", "application/json")
            .send_string(
                &json!({"data": {
                    "actor": "jane_example_com", "role": "user",
                    "body": "Check this requirement."
                }})
                .to_string(),
            ),
    )
    .status();
    assert_eq!(status, 200, "comment");
}

pub fn edit(host: &Host, statement: &str) {
    let record = get(host, "/requirements/req_review");
    let edited = response(
        request(host, "PATCH", "/requirements/req_review", true)
            .set("Content-Type", "application/json")
            .set(
                "If-Match",
                &format!("\"{}\"", record["data"]["edit"]["etag"].as_str().unwrap()),
            )
            .send_string(
                &json!({"data": {"actor": "jane_example_com", "statement": statement}}).to_string(),
            ),
    );
    let status = edited.status();
    assert_eq!(status, 200, "edit: {}", edited.into_string().unwrap());
    assert_eq!(
        get(host, "/requirements/req_review")["data"]["statement"],
        statement
    );
}
