#[path = "query_support/fixtures.rs"]
mod fixtures;

use provenance_macros::verifies;
use serde_json::json;

#[test]
#[verifies("rule_refuse_stale_names_the_moved_units", examples)]
fn refuse_stale_prints_one_error_line_to_stderr_and_exits_one() {
    let repo = fixtures::init_repo();
    let path = repo.path().to_str().unwrap();
    let request = json!({"node_type": "requirement", "id": "req_missing"});
    let before = fixtures::sdk(path, "get", &request);
    std::fs::write(repo.path().join(".provenance/state/change.txt"), "changed").unwrap();
    let output = fixtures::provenance()
        .args(["sdk", "get", "--repo", path, "--freshness", "refuse_stale"])
        .write_stdin(request.to_string())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("refuse_stale: the projection"), "{text}");
    assert!(
        text.contains(&format!("at serial {}", before["stamp"]["serial"])),
        "{text}"
    );
    for key in ["digest", "instance_id"] {
        assert!(
            text.contains(before["stamp"][key].as_str().unwrap()),
            "{text}"
        );
    }
    assert!(text.contains("moved: global (stored sha256:"), "{text}");
    assert!(text.contains(", live sha256:"), "{text}");
    assert!(
        text.contains("Read under catch_up or run `provenance materialize`."),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn refusal_escapes_repository_line_breaks() {
    let repo = fixtures::init_repo();
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("line\nbreak\rrepo");
    std::fs::rename(repo.path(), &root).unwrap();
    let path = root.to_str().unwrap();
    let request = json!({"node_type": "requirement", "id": "req_missing"});
    fixtures::sdk(path, "get", &request);
    std::fs::write(root.join(".provenance/state/change.txt"), "changed").unwrap();
    let output = fixtures::provenance()
        .args(["sdk", "get", "--repo", path, "--freshness", "refuse_stale"])
        .write_stdin(request.to_string())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("line\\nbreak\\rrepo"), "{text}");
}
