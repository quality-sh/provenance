use assert_cmd::Command;
use provenance_macros::verifies;
use std::path::{Path, PathBuf};

fn run(repo: &str, args: &[&str]) -> String {
    let result = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

fn create(repo: &str, id: &str, refines: Option<&str>) {
    let mut args = vec![
        "requirements",
        "create",
        "--id",
        id,
        "--statement",
        "The system is safe.",
    ];
    if let Some(parent) = refines {
        args.extend(["--refines", parent]);
    }
    run(repo, &args);
}

fn archive(repo: &str, id: &str) {
    let current: serde_json::Value = serde_json::from_str(&run(
        repo,
        &["api", &format!("requirements/{id}"), "--format", "json"],
    ))
    .unwrap();
    let etag = current["data"]["edit"]["etag"].as_str().unwrap();
    run(
        repo,
        &[
            id,
            "update",
            "--if-match",
            etag,
            "--status",
            "archived",
            "--archived-in-commit-json",
            r#"{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        ],
    );
}

/// The one Requirement shard of the default scope.
fn requirement_shard(root: &Path) -> PathBuf {
    let directory = root.join(".provenance/state/scopes/default/requirements");
    std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.file_name().unwrap() != "review.jsonl")
        .unwrap()
}

/// Gap entries of one kind, as `(node_id, related_node_id)`.
fn gaps_of_kind(repo: &str, kind: &str) -> Vec<(String, String)> {
    let output: serde_json::Value =
        serde_json::from_str(&run(repo, &["gaps", "--format", "json"])).unwrap();
    let items = output.get("data").unwrap_or(&output);
    items
        .as_array()
        .unwrap()
        .iter()
        .filter(|gap| gap["kind"] == kind)
        .map(|gap| {
            (
                gap["node_id"].as_str().unwrap().to_owned(),
                gap["related_node_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect()
}

/// One branch archives `req_parent`, and the other adds the live child
/// `req_child` under it. The merge of the two Requirement shards keeps both
/// changes, so `req_child` is live and its only parent is archived.
#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn gaps_report_a_live_requirement_whose_parents_a_merge_left_archived() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let repo = root.to_str().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    create(repo, "req_parent", None);
    let shard = requirement_shard(root);
    let sides = tempfile::tempdir().unwrap();
    let side = |name: &str| sides.path().join(name);
    std::fs::copy(&shard, side("base.jsonl")).unwrap();

    create(repo, "req_child", Some("req_parent"));
    std::fs::copy(&shard, side("theirs.jsonl")).unwrap();
    assert_eq!(
        gaps_of_kind(repo, "no_live_parent"),
        [] as [(String, String); 0]
    );

    std::fs::copy(side("base.jsonl"), &shard).unwrap();
    archive(repo, "req_parent");
    std::fs::copy(&shard, side("ours.jsonl")).unwrap();
    assert_eq!(
        gaps_of_kind(repo, "no_live_parent"),
        [] as [(String, String); 0]
    );

    let relative = shard.strip_prefix(root).unwrap().to_str().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "merge-jsonl",
            side("base.jsonl").to_str().unwrap(),
            side("ours.jsonl").to_str().unwrap(),
            side("theirs.jsonl").to_str().unwrap(),
            "--output",
            shard.to_str().unwrap(),
            "--path",
            &relative.replace('\\', "/"),
        ])
        .assert()
        .success();

    assert_eq!(
        gaps_of_kind(repo, "no_live_parent"),
        [("req_child".to_owned(), "req_parent".to_owned())]
    );
}
