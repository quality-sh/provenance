//! Implementation aid: no Rule states how often the review host starts Git. On Windows each
//! Git start from the detached host can show a console window, so this test keeps the count
//! of Git starts for a fixed request sequence at its bound (bead provenance-a5ru).
#![cfg(unix)]

#[path = "review_host_support/mod.rs"]
mod review_host_support;

use review_host_support::{
    follow::{create, pending_proposal},
    repository_with_actors, request, response, review_command, start_command, Host,
};
use serde_json::json;
use std::{os::unix::fs::PermissionsExt as _, path::Path};

/// Page loads in the sequence. Each load reads the configuration, the Requirement and
/// its document, as the review page does.
const PAGE_LOADS: usize = 5;

/// Statement edits in the sequence, after one submission decision and one comment.
const WRITES: usize = 2;

/// History reads of one Requirement in the sequence, after the writes.
const HISTORY_READS: usize = 3;

/// One reviewer identity read for the host process (`provenance.reviewer`, then
/// `user.email`) and one `rev-parse HEAD` for each write that stamps records. The
/// first history read starts `git --version` once for the process, then `rev-parse`,
/// `log`, and `cat-file` for the record file and for the Discussion file. A later
/// read at the same `HEAD` starts only `rev-parse`.
const GIT_START_BOUND: usize = 2 + WRITES + 5 + (HISTORY_READS - 1);

/// A `git` first on PATH that logs each start and then runs the real Git.
fn logging_git(directory: &Path, log: &Path) -> std::ffi::OsString {
    let real = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let real = String::from_utf8(real.stdout).unwrap();
    let wrapper = directory.join("git");
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
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(
        std::iter::once(directory.to_path_buf()).chain(std::env::split_paths(&path)),
    )
    .unwrap()
}

fn commit_all(repo: &Path) {
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-q",
            "-m",
            "start",
        ],
    ] {
        let status = provenance_store::git_process::git()
            .current_dir(repo)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }
}

fn get(host: &Host, path: &str) {
    let status = response(request(host, "GET", path, true).call()).status();
    assert_eq!(status, 200, "GET {path}");
}

fn accept(host: &Host, proposal: &str) {
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

fn comment(host: &Host) {
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

fn edit(host: &Host, statement: &str) {
    let record: serde_json::Value = serde_json::from_str(
        &response(request(host, "GET", "/requirements/req_review", true).call())
            .into_string()
            .unwrap(),
    )
    .unwrap();
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
}

#[test]
fn review_host_starts_git_a_bounded_number_of_times_for_page_loads_and_writes() {
    let repo = repository_with_actors(&["jane_example_com"]);
    create(repo.path(), "req_review", "requirement", &[]);
    commit_all(repo.path());
    let tools = tempfile::tempdir().unwrap();
    let log = tools.path().join("git-starts.log");
    let global = tools.path().join("gitconfig");
    std::fs::write(&global, "[user]\n\temail = jane@example.com\n").unwrap();
    let mut command = review_command(repo.path());
    command
        .env("PATH", logging_git(tools.path(), &log))
        .env("GIT_CONFIG_GLOBAL", &global)
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let host = start_command(command);

    for _ in 0..PAGE_LOADS {
        get(&host, "/review-config");
        get(&host, "/requirements/req_review");
        get(&host, "/requirements/req_review/document");
    }
    accept(&host, &pending_proposal(&host, "req_review"));
    comment(&host);
    edit(&host, "The system stores each result.");
    edit(&host, "The system keeps each result.");
    get(&host, "/review-config");
    for _ in 0..HISTORY_READS {
        get(&host, "/requirements/req_review/history");
    }

    let starts = std::fs::read_to_string(&log).unwrap_or_default();
    let count = starts.lines().count();
    eprintln!("review host Git starts: {count}\n{starts}");
    assert!(
        count <= GIT_START_BOUND,
        "the review host started Git {count} times, more than {GIT_START_BOUND}:\n{starts}"
    );
}
