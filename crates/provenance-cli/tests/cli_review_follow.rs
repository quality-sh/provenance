#[path = "review_host_support/mod.rs"]
mod review_host_support;

use review_host_support::{repository, start, waits};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    process::Stdio,
    sync::mpsc,
    time::Duration,
};

#[test]
// Implementation aid: this public characterization must fail before follow exists.
fn follow_returns_current_document_before_waiting_for_changes() {
    let repo = repository();
    let output = waits::output(
        std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance")).args([
            "req_follow",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The agent receives saved review changes.",
            "--repo",
            repo.path().to_str().unwrap(),
        ]),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let host = start(repo.path());
    let mut child = host
        .cli()
        .args([
            "req_follow",
            "get",
            "--view",
            "review",
            "--follow",
            "--format",
            "json",
            "--repo",
            repo.path().to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        let _ = send.send(line);
    });
    let line = receive.recv_timeout(Duration::from_secs(15));
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();
    let line = line.expect("the follow command must flush its current state");
    assert!(
        !line.is_empty(),
        "follow emitted no current state: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(state["type"], "state");
    assert!(state["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record.to_string().contains("req_follow")));
    assert!(state["epoch"].is_string());
    assert!(state["sequence"].is_u64());
    assert!(state["continuations"].is_array());
}

#[test]
// Implementation aid: external public writes must invalidate the selected document.
fn follow_receives_a_question_written_by_another_cli() {
    use review_host_support::follow::{create, Follow};
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    assert_eq!(follow.frame()["type"], "state");
    create(
        repo.path(),
        "q_follow",
        "question",
        &["--parent", "req_follow"],
    );
    let changed = follow.until("q_follow");
    assert_eq!(changed["type"], "state");
    assert!(changed["sequence"].as_u64().unwrap() > 0);
}
