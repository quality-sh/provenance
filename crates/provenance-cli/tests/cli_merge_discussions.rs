use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;

fn cli(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap()
}

fn json(repo: &Path, args: &[&str]) -> Value {
    let output = cli(repo, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn git(repo: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}

fn git_ok(repo: &Path, args: &[&str]) {
    let output = git(repo, args);
    assert!(
        output.status.success(),
        "git {args:?}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn commit(repo: &Path, message: &str) {
    git_ok(repo, &["add", "."]);
    git_ok(repo, &["commit", "-m", message]);
}

fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git_ok(repo, &["init", "--initial-branch", "main"]);
    git_ok(repo, &["config", "user.email", "test@example.test"]);
    git_ok(repo, &["config", "user.name", "Test"]);
    let binary = assert_cmd::cargo::cargo_bin!("provenance");
    git_ok(
        repo,
        &[
            "config",
            "merge.provenance-jsonl.driver",
            &format!(
                "'{}' merge-jsonl %O %A %B --output %A --path %P",
                binary.to_str().unwrap().replace('\\', "/")
            ),
        ],
    );
    std::fs::write(
        repo.join(".gitattributes"),
        ".provenance/state/**/*.jsonl merge=provenance-jsonl\n",
    )
    .unwrap();
    Command::new(binary)
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    json(
        repo,
        &[
            "requirements",
            "create",
            "--id",
            "req_a",
            "--statement",
            "A discussion has one parent.",
        ],
    );
    dir
}

fn discuss(repo: &Path, body: &str) -> String {
    json(repo, &["req_a", "discuss", "--body", body])["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .into()
}

fn reply(repo: &Path, id: &str, version: &str, body: &str) -> Value {
    json(
        repo,
        &[id, "reply", "--expected-version", version, "--body", body],
    )
}

fn assert_readable(repo: &Path, ids: &[&str]) -> usize {
    let list = json(repo, &["discussions"]);
    assert_eq!(
        list["result"]["entries"].as_array().unwrap().len(),
        ids.len()
    );
    let mut message_ids = std::collections::BTreeSet::new();
    for id in ids {
        let get = json(repo, &["discussions", id, "get"]);
        let head_ids = get["result"]["head"]["message_ids"].as_array().unwrap();
        let messages = get["result"]["messages"]["entries"].as_array().unwrap();
        assert_eq!(messages.len(), head_ids.len());
        for message in messages {
            assert!(head_ids.contains(&message["id"]));
            assert!(message_ids.insert(message["id"].as_str().unwrap().to_owned()));
        }
    }
    message_ids.len()
}

#[test]
#[verifies("rule_message_id_is_random", examples)]
#[verifies("rule_discussion_merge_keys_discussion_id", examples)]
fn replies_and_new_discussions_on_two_branches_merge_and_load() {
    let dir = repository();
    let repo = dir.path();
    let x = discuss(repo, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let y = discuss(repo, "theirs");
    reply(repo, &y, "1", "their reply");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    reply(repo, &x, "1", "our reply");
    commit(repo, "ours");
    git_ok(repo, &["merge", "theirs", "-m", "merge"]);
    assert_eq!(assert_readable(repo, &[&x, &y]), 4);
    reply(repo, &x, "2", "after merge");
    assert_eq!(assert_readable(repo, &[&x, &y]), 5);
}

#[test]
#[verifies("rule_message_id_is_random", examples)]
#[verifies("rule_discussion_merge_keys_discussion_id", examples)]
fn two_first_discussions_share_the_identical_thread_after_merge() {
    let dir = repository();
    let repo = dir.path();
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let y = discuss(repo, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    let x = discuss(repo, "ours");
    commit(repo, "ours");
    git_ok(repo, &["merge", "theirs", "-m", "merge"]);
    assert_eq!(assert_readable(repo, &[&x, &y]), 2);
    reply(repo, &x, "1", "after merge");
    assert_eq!(assert_readable(repo, &[&x, &y]), 3);
}

#[test]
#[verifies("rule_discussion_merge_keys_discussion_id", examples)]
fn replies_to_one_discussion_report_a_typed_conflict() {
    let dir = repository();
    let repo = dir.path();
    let id = discuss(repo, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    reply(repo, &id, "1", "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    reply(repo, &id, "1", "ours");
    commit(repo, "ours");
    let output = git(repo, &["merge", "theirs", "-m", "merge"]);
    assert!(!output.status.success());
    let diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(diagnostic.contains("divergent_edit"), "{diagnostic}");
    assert!(diagnostic.contains(&id), "{diagnostic}");
}

fn rows(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn write_rows(path: &Path, rows: &[Value]) {
    let text = rows
        .iter()
        .map(|row| serde_json::to_string(row).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, text + "\n").unwrap();
}

#[test]
#[verifies("rule_duplicate_message_id_names_lines", examples)]
fn duplicate_message_diagnostics_and_documented_hand_repair() {
    let dir = repository();
    let repo = dir.path();
    let x = discuss(repo, "first Discussion");
    let y = discuss(repo, "second Discussion");
    let threads = repo.join(".provenance/state/scopes/default/threads");
    let shard = std::fs::read_dir(&threads)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
                && name != "threads.jsonl"
                && name != "discussions.jsonl"
        })
        .unwrap();
    let mut messages = rows(&shard);
    assert_eq!(messages.len(), 2);
    for message in &mut messages {
        message["id"] = Value::from("msg_000021");
    }
    write_rows(&shard, &messages);
    let discussions_path = threads.join("discussions.jsonl");
    let mut discussions = rows(&discussions_path);
    for discussion in &mut discussions {
        discussion["message_ids"] = serde_json::json!(["msg_000021"]);
        discussion["root_message_id"] = Value::from("msg_000021");
    }
    write_rows(&discussions_path, &discussions);

    for args in [
        vec!["req_a", "discuss", "--body", "refused"],
        vec![
            x.as_str(),
            "reply",
            "--expected-version",
            "1",
            "--body",
            "refused",
        ],
        vec!["check"],
    ] {
        let output = cli(repo, &args);
        assert_eq!(output.status.code(), Some(1));
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            diagnostic.contains("Message ID msg_000021 appears more than once:"),
            "{diagnostic}"
        );
        for line in [1, 2] {
            assert!(
                diagnostic.contains(&format!(
                    "{}:{line}",
                    shard.file_name().unwrap().to_str().unwrap()
                )),
                "{diagnostic}"
            );
        }
        assert!(
            diagnostic.contains("docs/cli.md#repairing-a-duplicate-message-id"),
            "{diagnostic}"
        );
    }
    let read = cli(repo, &["discussions"]);
    let diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&read.stdout),
        String::from_utf8_lossy(&read.stderr)
    );
    assert!(
        diagnostic.contains("Message ID msg_000021 appears more than once:"),
        "{diagnostic}"
    );

    let replacement = format!("msg_{}", uuid::Uuid::new_v4());
    messages[1]["id"] = Value::from(replacement.clone());
    write_rows(&shard, &messages);
    let owner = discussions
        .iter_mut()
        .find(|discussion| discussion["discussion_id"] == y)
        .unwrap();
    owner["root_message_id"] = Value::from(replacement.clone());
    owner["message_ids"] = serde_json::json!([replacement]);
    write_rows(&discussions_path, &discussions);
    assert_eq!(assert_readable(repo, &[&x, &y]), 2);
    let z = discuss(repo, "after repair");
    reply(repo, &x, "1", "after repair");
    assert_eq!(assert_readable(repo, &[&x, &y, &z]), 4);
    json(repo, &["check"]);
}
