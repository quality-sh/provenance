#![cfg(feature = "dogfood")]

//! Triage behavior: mark captured notes handled and reopen them without ever
//! rewriting the capture spool. Every test runs against an isolated temporary
//! spool via `PROVENANCE_DOGFOOD_DIR`; the live `~/.provenance/dogfood` spool
//! is never touched.

use assert_cmd::Command;
use std::path::Path;
use std::process::{Command as RawCommand, Stdio};

fn dogfood_cmd(spool_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("provenance").unwrap();
    cmd.env("PROVENANCE_DOGFOOD_DIR", spool_dir)
        .env_remove("PROVENANCE_SESSION_ID")
        .env_remove("WORKFLOWD_SESSION_ID")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("OPENCODE_SESSION_ID");
    cmd
}

fn capture_note(spool_dir: &Path, summary: &str) {
    dogfood_cmd(spool_dir)
        .args([
            "dogfood",
            "note",
            "--surface",
            "general",
            "--category",
            "friction",
            "--severity",
            "annoyance",
            summary,
        ])
        .assert()
        .success();
}

/// A handcrafted note line in the pre-triage legacy shape: only the fields
/// capture has always written, keys in an unusual order, plus an unknown
/// field a newer writer might add. Readers must accept it untouched.
fn legacy_note_line(ts_ms: i64, summary: &str) -> String {
    format!(
        r#"{{"summary":"{summary}","extra_unknown_field":true,"surface":"prime","severity":"blocked","ts_ms":{ts_ms},"category":"confusion","host":"legacy-host","provenance_version":"0.1.0","session_id":null,"repo":null,"branch":null,"commit":null,"detail":null,"suggestion":null}}"#
    )
}

fn seed_spool(spool_dir: &Path, summaries: &[&str]) {
    std::fs::create_dir_all(spool_dir).unwrap();
    let mut spool = String::new();
    for (index, summary) in summaries.iter().enumerate() {
        let ts = i64::try_from(1_700_000_000_000 + index as u128).unwrap();
        spool.push_str(&legacy_note_line(ts, summary));
        spool.push('\n');
    }
    std::fs::write(spool_dir.join("notes.jsonl"), spool).unwrap();
}

fn triage_list_entries(spool_dir: &Path) -> Vec<serde_json::Value> {
    let output = dogfood_cmd(spool_dir)
        .args(["dogfood", "triage", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn entries_with_status(spool_dir: &Path, status: &str) -> Vec<serde_json::Value> {
    let output = dogfood_cmd(spool_dir)
        .args(["dogfood", "triage", "list", "--status", status])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn handle_marks_note_handled_persists_and_reopens() {
    let temp = tempfile::tempdir().unwrap();
    seed_spool(temp.path(), &["prime output truncates rule statements"]);

    // Fresh notes start unhandled, with a stable-looking identifier.
    let entries = triage_list_entries(temp.path());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["status"], "unhandled");
    assert!(entries[0]["reason"].is_null());
    let id = entries[0]["id"].as_str().unwrap();
    assert_eq!(id.len(), 16, "id is a fixed-width hex identifier");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(
        entries[0]["summary"],
        "prime output truncates rule statements"
    );

    dogfood_cmd(temp.path())
        .args([
            "dogfood",
            "triage",
            "handle",
            id,
            "--reason",
            "fixed by provenance-9mvv",
        ])
        .assert()
        .success();

    // A separate invocation must observe the persisted state.
    let handled = entries_with_status(temp.path(), "handled");
    assert_eq!(handled.len(), 1);
    assert_eq!(handled[0]["id"], id);
    assert_eq!(handled[0]["reason"], "fixed by provenance-9mvv");
    assert!(entries_with_status(temp.path(), "unhandled").is_empty());

    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "reopen", id])
        .assert()
        .success();

    let unhandled = entries_with_status(temp.path(), "unhandled");
    assert_eq!(unhandled.len(), 1);
    assert_eq!(unhandled[0]["id"], id);
    assert!(unhandled[0]["reason"].is_null(), "reopen clears the reason");

    // The identifier survives the state changes: it identifies the note,
    // not its triage state.
    let entries_after = triage_list_entries(temp.path());
    assert_eq!(entries_after[0]["id"], id);
}

#[test]
fn handle_accepts_unambiguous_id_prefix() {
    let temp = tempfile::tempdir().unwrap();
    seed_spool(temp.path(), &["first note", "second note"]);

    let entries = triage_list_entries(temp.path());
    let id = entries[0]["id"].as_str().unwrap();
    let prefix: String = id.chars().take(8).collect();
    assert_ne!(prefix, id);

    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "handle", &prefix])
        .assert()
        .success();

    let handled = entries_with_status(temp.path(), "handled");
    assert_eq!(handled.len(), 1);
    assert_eq!(handled[0]["id"], id);
    assert!(handled[0]["reason"].is_null(), "reason is optional");
}

#[test]
fn handle_refuses_unknown_identifier_without_touching_state() {
    let temp = tempfile::tempdir().unwrap();
    seed_spool(temp.path(), &["a real note"]);

    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "handle", "ffffffffffffffff"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown"));

    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "reopen", "ffffffffffffffff"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown"));

    assert!(
        !temp.path().join("triage.jsonl").exists(),
        "a refused triage command must not write state"
    );
    assert_eq!(triage_list_entries(temp.path())[0]["status"], "unhandled");
}

#[test]
fn handle_refuses_ambiguous_prefix() {
    // Seventeen notes guarantee (pigeonhole) that two ids share a first hex
    // digit, so a one-character prefix is guaranteed to be ambiguous.
    let summaries: Vec<String> = (0..17).map(|i| format!("note number {i}")).collect();
    let summary_refs: Vec<&str> = summaries.iter().map(String::as_str).collect();
    let temp = tempfile::tempdir().unwrap();
    seed_spool(temp.path(), &summary_refs);

    let entries = triage_list_entries(temp.path());
    let ids: Vec<&str> = entries.iter().map(|e| e["id"].as_str().unwrap()).collect();
    let mut ambiguous_prefix = None;
    for (i, id) in ids.iter().enumerate() {
        for other in &ids[i + 1..] {
            if id.as_bytes()[0] == other.as_bytes()[0] {
                ambiguous_prefix = Some((id.chars().next().unwrap().to_string(), *id, *other));
            }
        }
    }
    let (prefix, id_a, id_b) = ambiguous_prefix.expect("pigeonhole guarantees a collision");

    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "handle", &prefix])
        .assert()
        .failure()
        .stderr(predicates::str::contains("ambiguous"))
        .stderr(predicates::str::contains(id_a))
        .stderr(predicates::str::contains(id_b));

    assert_eq!(entries_with_status(temp.path(), "handled").len(), 0);
}

#[test]
fn triage_never_rewrites_the_note_spool_and_keeps_list_compatible() {
    let temp = tempfile::tempdir().unwrap();
    capture_note(temp.path(), "captured first");
    capture_note(temp.path(), "captured second");

    let spool_path = temp.path().join("notes.jsonl");
    let bytes_before = std::fs::read(&spool_path).unwrap();

    let entries = triage_list_entries(temp.path());
    let id = entries[0]["id"].as_str().unwrap().to_string();
    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "handle", &id, "--reason", "triaged"])
        .assert()
        .success();
    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "reopen", &id])
        .assert()
        .success();

    let bytes_after = std::fs::read(&spool_path).unwrap();
    assert_eq!(
        bytes_before, bytes_after,
        "triage must byte-preserve the capture spool"
    );

    // The plain capture-history list is unchanged by triage activity.
    let output = dogfood_cmd(temp.path())
        .args(["dogfood", "list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let notes: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let notes = notes.as_array().unwrap();
    assert_eq!(notes.len(), 2);
    for note in notes {
        assert!(
            note.get("triage").is_none() && note.get("status").is_none(),
            "capture-history output must stay free of triage fields"
        );
    }
}

#[test]
fn triage_identity_is_stable_for_legacy_notes_regardless_of_line_formatting() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path()).unwrap();
    // The same logical note twice: once in legacy key order with an unknown
    // extra field, once compact without it. Identity must come from the
    // parsed note, never from raw bytes, key order, or list position.
    let legacy = legacy_note_line(1_700_000_000_042, "same pain twice");
    let canonical = r#"{"ts_ms":1700000000042,"session_id":null,"host":"legacy-host","repo":null,"branch":null,"commit":null,"provenance_version":"0.1.0","surface":"prime","category":"confusion","severity":"blocked","summary":"same pain twice","detail":null,"suggestion":null}"#;
    std::fs::write(
        temp.path().join("notes.jsonl"),
        format!("{legacy}\n{canonical}\n"),
    )
    .unwrap();

    let entries = triage_list_entries(temp.path());
    assert_eq!(
        entries.len(),
        1,
        "identical logical notes share one triage identity"
    );
    let id = entries[0]["id"].as_str().unwrap();

    // Stability across invocations: a second read sees the same identifier.
    assert_eq!(triage_list_entries(temp.path())[0]["id"], id);

    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "handle", id])
        .assert()
        .success();
    let handled = entries_with_status(temp.path(), "handled");
    assert_eq!(handled.len(), 1);
    assert_eq!(handled[0]["id"], id);
}

#[test]
fn triage_list_filters_unhandled_handled_and_all() {
    let temp = tempfile::tempdir().unwrap();
    seed_spool(temp.path(), &["stays open", "gets triaged"]);

    let entries = triage_list_entries(temp.path());
    let to_handle = entries
        .iter()
        .find(|e| e["summary"] == "gets triaged")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "handle", &to_handle])
        .assert()
        .success();

    assert_eq!(entries_with_status(temp.path(), "all").len(), 2);
    let unhandled = entries_with_status(temp.path(), "unhandled");
    assert_eq!(unhandled.len(), 1);
    assert_eq!(unhandled[0]["summary"], "stays open");
    let handled = entries_with_status(temp.path(), "handled");
    assert_eq!(handled.len(), 1);
    assert_eq!(handled[0]["summary"], "gets triaged");

    // The default view shows every note.
    let default_output = dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let default_entries: Vec<serde_json::Value> = serde_json::from_slice(&default_output).unwrap();
    assert_eq!(default_entries.len(), 2);
}

#[test]
fn concurrent_triage_commands_do_not_lose_updates() {
    let summaries: Vec<String> = (0..6).map(|i| format!("concurrent note {i}")).collect();
    let summary_refs: Vec<&str> = summaries.iter().map(String::as_str).collect();
    let temp = tempfile::tempdir().unwrap();
    seed_spool(temp.path(), &summary_refs);

    let ids: Vec<String> = triage_list_entries(temp.path())
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();

    let mut children = Vec::new();
    for id in &ids {
        let child = RawCommand::new(env!("CARGO_BIN_EXE_provenance"))
            .env("PROVENANCE_DOGFOOD_DIR", temp.path())
            .args(["dogfood", "triage", "handle", id])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn concurrent triage command");
        children.push(child);
    }
    for mut child in children {
        let status = child.wait().expect("wait for triage command");
        assert!(status.success(), "every concurrent triage command succeeds");
    }

    let triage_path = temp.path().join("triage.jsonl");
    let raw = std::fs::read_to_string(&triage_path).unwrap();
    let mut lines = 0;
    for line in raw.lines() {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|e| panic!("state line {lines} must be intact: {e}"));
        lines += 1;
    }
    assert_eq!(lines, 6, "every update is recorded, none is lost");
    assert_eq!(entries_with_status(temp.path(), "handled").len(), 6);
}

#[test]
fn torn_triage_state_line_is_skipped_with_a_warning() {
    let temp = tempfile::tempdir().unwrap();
    seed_spool(temp.path(), &["torn state target"]);
    let id = triage_list_entries(temp.path())[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "handle", &id, "--reason", "done"])
        .assert()
        .success();

    // Simulate a torn write at the end of the state file.
    let triage_path = temp.path().join("triage.jsonl");
    let mut state = std::fs::read_to_string(&triage_path).unwrap();
    state.push_str(r#"{"ts_ms":17000000,"note_id":"ab"#);
    std::fs::write(&triage_path, state).unwrap();

    let assert = dogfood_cmd(temp.path())
        .args(["dogfood", "triage", "list", "--status", "handled"])
        .assert()
        .success();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("skipping"),
        "a torn state line warns instead of failing: {stderr}"
    );
    let stdout = assert.get_output().stdout.clone();
    let handled: Vec<serde_json::Value> = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(handled.len(), 1);
    assert_eq!(handled[0]["reason"], "done");
}
