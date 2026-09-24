use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .output()
        .unwrap()
}

fn success(args: &[&str]) -> std::process::Output {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn json(args: &[&str]) -> Value {
    serde_json::from_slice(&success(args).stdout).unwrap()
}

fn repo_with_requirement() -> tempfile::TempDir {
    let help = String::from_utf8(success(&["--help"]).stdout).unwrap();
    assert!(help.contains("discussions"));
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    success(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    success(&[
        "requirements",
        "create",
        "--repo",
        repo,
        "--id",
        "req_a",
        "--statement",
        "A discussion has one parent.",
    ]);
    let path = directory
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let content = std::fs::read_to_string(&path).unwrap();
    let mut legacy: Value = serde_json::from_str(content.lines().next().unwrap()).unwrap();
    legacy["id"] = Value::String("discussions".into());
    legacy["schema_version"] = Value::from(2);
    std::fs::write(
        path,
        format!("{}\n{content}", serde_json::to_string(&legacy).unwrap()),
    )
    .unwrap();
    let keyword_record = json(&["discussions", "get", "--repo", repo, "--format", "json"]);
    assert_eq!(keyword_record["record"]["id"], "discussions");
    let member = json(&[
        "requirements",
        "discussions",
        "get",
        "--repo",
        repo,
        "--format",
        "json",
    ]);
    let updated = json(&[
        "discussions",
        "update",
        "--repo",
        repo,
        "--statement",
        "A legacy graph ID remains writable.",
        "--if-match",
        member["data"]["edit"]["etag"].as_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(updated["data"]["id"], "discussions");
    directory
}

#[test]
fn discussion_words_are_reserved_for_new_records() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    success(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    for word in ["discussions", "discussion", "discuss", "reply"] {
        let output = run(&[
            "requirements",
            "create",
            "--repo",
            repo,
            "--id",
            word,
            "--statement",
            "A new record cannot use a command word.",
        ]);
        assert!(!output.status.success(), "{word}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("reserved record ID"));
    }
}

#[test]
fn discussion_help_and_errors_keep_the_discussion_grammar() {
    let help = success(&["discussions", "--help"]);
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("List or read addressed Discussions"), "{text}");
    let invalid = run(&["discussions", "--status", "draft"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("possible values"));
    let addressed = run(&["discussions", "some_id", "invented"]);
    assert!(!addressed.status.success());
    assert!(String::from_utf8_lossy(&addressed.stderr).contains("possible values: get"));
    let legacy = run(&["discussions", "update", "--unknown-option"]);
    assert!(!legacy.status.success());
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("--unknown-option"));
}

#[test]
fn target_status_uses_the_selected_record_contract() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    success(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    success(&[
        "requirements",
        "create",
        "--repo",
        repo,
        "--id",
        "req_status",
        "--statement",
        "A record has its own lifecycle.",
    ]);
    let resolution = json(&[
        "res_status",
        "create",
        "--type",
        "resolution",
        "--repo",
        repo,
        "--title",
        "Lifecycle",
        "--requirement-id",
        "req_status",
        "--position",
        "Use the record status.",
        "--rationale",
        "The graph records the choice.",
        "--status",
        "proposed",
        "--format",
        "json",
    ]);
    assert_eq!(resolution["data"]["status"], "proposed");
    let rule = json(&[
        "rule_status",
        "create",
        "--type",
        "rule",
        "--repo",
        repo,
        "--requirement-id",
        "req_status",
        "--statement",
        "A rule has a lifecycle.",
        "--severity",
        "medium",
        "--status",
        "draft",
        "--format",
        "json",
    ]);
    assert_eq!(rule["data"]["status"], "draft");
    let invalid = run(&[
        "rule_status",
        "update",
        "--repo",
        repo,
        "--status",
        "resolved",
    ]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid value for --status"));
}

fn resolve_second_discussion(directory: &tempfile::TempDir, second_id: &str) {
    let store = provenance_store::state_store::StateStore::new(
        provenance_store::layout::ProvenanceLayout::new(
            camino::Utf8Path::from_path(directory.path()).unwrap(),
        ),
    );
    store
        .write_discussion(provenance_store::review::WriteDiscussion {
            scope_id: provenance_core::ScopeId::new("default").unwrap(),
            parent: provenance_core::ThreadParent {
                node_type: provenance_core::NodeType::Requirement,
                node_id: provenance_core::StableId::new("req_a").unwrap(),
            },
            request_id: provenance_core::StableId::new("resolve_second").unwrap(),
            actor: "cli".into(),
            declared_by: None,
            action: provenance_store::review::DiscussionAction::SetStatus {
                discussion_id: provenance_core::StableId::new(second_id).unwrap(),
                expected_version: 1,
                status: provenance_core::threads::DiscussionStatus::Resolved,
            },
        })
        .unwrap();
}

#[test]
#[verifies("rule_porcelain_discussion_targets", examples)]
fn cli_discussion_actions_use_one_scope_and_preserve_receipt_identity() {
    let directory = repo_with_requirement();
    let repo = directory.path().to_str().unwrap();
    let start = json(&[
        "req_a",
        "discuss",
        "--repo",
        repo,
        "--body",
        "Opening text",
        "--format",
        "json",
    ]);
    assert_eq!(start["kind"], "written");
    assert_eq!(start["receipt"]["version"], 1);
    let id = start["receipt"]["discussion_id"].as_str().unwrap();
    let second = json(&[
        "req_a",
        "discuss",
        "--repo",
        repo,
        "--body",
        "Resolved text",
        "--format",
        "json",
    ]);
    let second_id = second["receipt"]["discussion_id"].as_str().unwrap();
    resolve_second_discussion(&directory, second_id);
    let scope = json(&["discussions", "--repo", repo, "--format", "json"]);
    let prefixed = json(&["--repo", repo, "discussions", "--format", "json"]);
    assert_eq!(prefixed["result"]["entries"], scope["result"]["entries"]);
    assert_eq!(scope["scope_id"], "default");
    assert_eq!(scope["status"], "active");
    assert_eq!(scope["result"]["entries"][0]["discussion_id"], id);
    assert_eq!(scope["result"]["entries"].as_array().unwrap().len(), 1);
    let resolved = json(&[
        "discussions",
        "--repo",
        repo,
        "--status",
        "resolved",
        "--format",
        "json",
    ]);
    assert_eq!(resolved["result"]["entries"][0]["discussion_id"], second_id);
    let all = json(&[
        "discussions",
        "--repo",
        repo,
        "--status",
        "all",
        "--limit",
        "1",
        "--format",
        "json",
    ]);
    assert_eq!(all["has_more"], true);
    let parent = json(&["req_a", "discussions", "--repo", repo, "--format", "json"]);
    assert_eq!(parent["result"]["entries"][0]["discussion_id"], id);
    let invalid_status = run(&[
        "req_a", "discussions", "--repo", repo, "--status", "draft",
    ]);
    assert!(!invalid_status.status.success());
    assert!(String::from_utf8_lossy(&invalid_status.stderr).contains("status"));
    let conversation = json(&["discussions", id, "get", "--repo", repo, "--format", "json"]);
    assert_eq!(conversation["result"]["head"]["version"], 1);
    let reply = json(&[
        id,
        "reply",
        "--repo",
        repo,
        "--body",
        "Second message",
        "--expected-version",
        "1",
        "--format",
        "json",
    ]);
    assert!(reply["receipt"]["request_id"].is_string());
    assert_eq!(reply["receipt"]["version"], 2);
    let get = json(&["req_a", "get", "--repo", repo, "--format", "json"]);
    assert_eq!(get["record"]["id"], "req_a");
}
