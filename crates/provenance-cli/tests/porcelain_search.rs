use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn run(arguments: &[&str]) -> std::process::Output {
    provenance().args(arguments).output().unwrap()
}

fn success(arguments: &[&str]) -> std::process::Output {
    let output = run(arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn json(arguments: &[&str]) -> Value {
    serde_json::from_slice(&success(arguments).stdout).unwrap()
}

fn seed(repo: &str) {
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
        "sources",
        "create",
        "--repo",
        repo,
        "--id",
        "source_search",
        "--name",
        "Archive needle source",
    ]);
    success(&[
        "requirements",
        "create",
        "--repo",
        repo,
        "--id",
        "req_search",
        "--statement",
        "Shared needle requirement.",
    ]);
    success(&[
        "rules",
        "create",
        "--repo",
        repo,
        "--id",
        "rule_search",
        "--requirement-id",
        "req_search",
        "--statement",
        "Shared needle rule.",
        "--severity",
        "medium",
    ]);
}

fn ids(page: &Value) -> Vec<&str> {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect()
}

#[test]
#[verifies("rule_porcelain_search_text_optional", examples)]
#[verifies("rule_porcelain_search_predicates_intersect", examples)]
#[verifies("rule_porcelain_search_crosses_kinds", examples)]
fn root_search_supports_text_kind_and_intersection_in_both_formats() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    seed(repo);

    let text_only = json(&[
        "search", "--repo", repo, "--text", "archive", "--format", "json",
    ]);
    assert_eq!(ids(&text_only), ["source_search"]);

    let kind_only = json(&[
        "search",
        "--repo",
        repo,
        "--kind",
        "requirement",
        "--kind",
        "rule",
        "--format",
        "json",
    ]);
    assert_eq!(ids(&kind_only), ["req_search", "rule_search"]);

    let combined = json(&[
        "search", "--repo", repo, "--text", "shared", "--kind", "source", "--kind", "rule",
        "--limit", "1", "--format", "json",
    ]);
    assert_eq!(ids(&combined), ["rule_search"]);
    assert_eq!(combined["limit"], 1);
    assert_eq!(combined["has_more"], false);
    assert!(combined["next_cursor"].is_null());

    let readable = String::from_utf8(
        success(&[
            "search", "--repo", repo, "--text", "shared", "--kind", "rule", "--limit", "1",
        ])
        .stdout,
    )
    .unwrap();
    assert!(readable.contains("rule_search"), "{readable}");
    assert!(readable.contains("limit=1"), "{readable}");
    assert!(readable.contains("has_more=false"), "{readable}");
    assert!(readable.contains("continuation=none"), "{readable}");
}

#[test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
fn continuation_has_no_loss_and_refuses_wrong_query_or_revision() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    seed(repo);
    let first = json(&[
        "search",
        "--repo",
        repo,
        "--kind",
        "requirement",
        "--kind",
        "rule",
        "--limit",
        "1",
        "--format",
        "json",
    ]);
    assert_eq!(first["limit"], 1);
    assert_eq!(first["has_more"], true);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = json(&[
        "search",
        "--repo",
        repo,
        "--kind",
        "requirement",
        "--kind",
        "rule",
        "--limit",
        "1",
        "--cursor",
        cursor,
        "--format",
        "json",
    ]);
    let mut found = ids(&first);
    found.extend(ids(&second));
    assert_eq!(found, ["req_search", "rule_search"]);
    assert_eq!(second["has_more"], false);
    assert!(second["next_cursor"].is_null());

    let wrong_query = run(&[
        "search",
        "--repo",
        repo,
        "--text",
        "different",
        "--kind",
        "requirement",
        "--kind",
        "rule",
        "--limit",
        "1",
        "--cursor",
        cursor,
    ]);
    assert!(!wrong_query.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_query.stderr).contains("cursor"),
        "{}",
        String::from_utf8_lossy(&wrong_query.stderr)
    );

    success(&[
        "sources",
        "create",
        "--repo",
        repo,
        "--id",
        "source_revision_change",
        "--name",
        "Revision change",
    ]);
    let stale = run(&[
        "search",
        "--repo",
        repo,
        "--kind",
        "requirement",
        "--kind",
        "rule",
        "--limit",
        "1",
        "--cursor",
        cursor,
    ]);
    assert!(!stale.status.success());
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("revision"),
        "{}",
        String::from_utf8_lossy(&stale.stderr)
    );
}

#[test]
fn search_help_and_reserved_target_routing_are_deterministic() {
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
        "sources",
        "create",
        "--repo",
        repo,
        "--id",
        "search",
        "--name",
        "Reserved search target",
    ]);

    let help = String::from_utf8(success(&["search", "--help"]).stdout).unwrap();
    for option in ["--text", "--kind", "--limit", "--cursor", "--format"] {
        assert!(help.contains(option), "{help}");
    }
    let target = json(&["search", "get", "--repo", repo, "--format", "json"]);
    assert_eq!(target["record"]["id"], "search");
    success(&["sources", "--help", "--repo", repo]);
}
