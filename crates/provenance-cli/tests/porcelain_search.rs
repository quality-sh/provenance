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
        "source_search",
        "create",
        "--type",
        "source",
        "--repo",
        repo,
        "--name",
        "Archive needle source",
    ]);
    success(&[
        "req_search",
        "create",
        "--type",
        "requirement",
        "--repo",
        repo,
        "--statement",
        "Shared needle requirement.",
    ]);
    success(&[
        "rule_search",
        "create",
        "--type",
        "rule",
        "--repo",
        repo,
        "--requirement-id",
        "req_search",
        "--statement",
        "Shared needle rule.",
        "--status",
        "active",
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
        "source_revision_change",
        "create",
        "--type",
        "source",
        "--repo",
        repo,
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
fn search_help_is_available_without_a_repository() {
    let directory = tempfile::tempdir().unwrap();
    let output = provenance()
        .current_dir(directory.path())
        .args(["search", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for option in ["--text", "--kind", "--limit", "--cursor", "--format"] {
        assert!(help.contains(option), "{help}");
    }
}

fn rule(repo: &str, id: &str, statement: &str, status: &str) {
    success(&[
        id,
        "create",
        "--type",
        "rule",
        "--repo",
        repo,
        "--requirement-id",
        "req_search",
        "--statement",
        statement,
        "--status",
        status,
        "--severity",
        "medium",
    ]);
}

/// A repository with two Rules bound in `code.rs`: one to `pay`, one to `other`.
fn seed_code(repo: &str) {
    seed(repo);
    rule(
        repo,
        "rule_pay",
        "Pay overtime after the threshold.",
        "active",
    );
    rule(repo, "rule_other", "Other pay rule.", "deprecated");
    std::fs::write(
        std::path::Path::new(repo).join("code.rs"),
        concat!(
            "#[rule(\"rule_other\")]\n",
            "fn other() {}\n",
            "#[rule(\"rule_pay\")]\n",
            "fn pay() {}\n",
            "#[verifies(\"rule_pay\", examples)]\n",
            "fn checks_pay() {}\n",
        ),
    )
    .unwrap();
}

fn file_search(repo: &str, extra: &[&str]) -> Value {
    let mut arguments = vec!["search", "--repo", repo, "--file", "code.rs"];
    arguments.extend_from_slice(extra);
    arguments.extend_from_slice(&["--format", "json"]);
    json(&arguments)
}

#[test]
#[verifies("rule_search_file_returns_bound_rules", examples)]
fn file_search_returns_bound_rules_symbol_first_like_resolve_symbol() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    seed_code(repo);

    let page = file_search(repo, &["--symbol", "pay"]);
    assert_eq!(ids(&page), ["rule_pay", "rule_other"]);
    assert_eq!(page["operation"], "resolve-symbol");
    assert!(page["stamp"].is_object());
    assert_eq!(page["file"], "code.rs");
    assert_eq!(page["symbol"], "pay");
    assert_eq!(page["limit"], 50);
    assert_eq!(page["has_more"], false);

    let resolved = json(&[
        "api",
        "rules",
        "--query",
        "query=resolve-symbol",
        "--repo",
        repo,
        "--query",
        "file=code.rs",
        "--query",
        "symbol=pay",
        "--format",
        "json",
    ]);
    let data = &resolved["data"];
    assert_eq!(page["nodes"], data["items"]);
    assert_eq!(page["matches"], data["matches"]);
    assert_eq!(page["file"], data["file"]);
    assert_eq!(page["symbol"], data["symbol"]);

    let readable = String::from_utf8(
        success(&[
            "search", "--repo", repo, "--file", "code.rs", "--symbol", "pay",
        ])
        .stdout,
    )
    .unwrap();
    assert!(
        readable
            .starts_with("search: 2 returned for code.rs symbol pay\n- rule rule_pay · active\n"),
        "{readable}"
    );
    assert!(
        readable.contains("  · implementation line 3 pay (symbol match)\n"),
        "{readable}"
    );
    assert!(
        readable.contains("  · verification line 5 checks_pay examples (file match)\n"),
        "{readable}"
    );
    assert!(
        readable.contains("- rule rule_other · deprecated\n"),
        "{readable}"
    );

    let terminal = file_search(repo, &["--symbol", "pay", "--include-terminal"]);
    assert_eq!(terminal["nodes"], page["nodes"]);
}

#[test]
#[verifies("rule_search_file_returns_bound_rules", examples)]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
fn file_search_pages_continue_with_the_cursor() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    seed_code(repo);

    let first = file_search(repo, &["--symbol", "pay", "--limit", "1"]);
    assert_eq!(ids(&first), ["rule_pay"]);
    assert_eq!(first["has_more"], true);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = file_search(
        repo,
        &["--symbol", "pay", "--limit", "1", "--cursor", cursor],
    );
    assert_eq!(ids(&second), ["rule_other"]);
    assert_eq!(second["has_more"], false);
}

#[test]
#[verifies("rule_search_file_refuses_other_filters", examples)]
fn file_search_refuses_other_filters_and_a_symbol_without_a_file() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    seed_code(repo);

    for (arguments, field) in [
        (&["--file", "code.rs", "--text", "pay"][..], "text"),
        (
            &["--file", "code.rs", "--kind", "requirement"][..],
            "node_types",
        ),
        (&["--symbol", "pay"][..], "symbol"),
        (&["--kind", "rule", "--line", "3"][..], "line"),
    ] {
        let mut command = vec!["search", "--repo", repo];
        command.extend_from_slice(arguments);
        let output = run(&command);
        assert!(!output.status.success(), "{arguments:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&format!("{field}: ")), "{stderr}");
        assert!(
            stderr.contains("provenance search --file <path>"),
            "{stderr}"
        );
    }
}

// Unbound characterization test: no Rule states that code lookup keeps the text
// search output the same. This test keeps the existing JSON keys of text search.
#[test]
fn text_search_json_has_no_code_lookup_fields() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    seed(repo);

    let page = json(&[
        "search", "--repo", repo, "--text", "shared", "--format", "json",
    ]);
    let mut keys = page
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "has_more",
            "limit",
            "next_cursor",
            "nodes",
            "operation",
            "protocol_version",
            "stamp"
        ]
    );
    assert_eq!(page["operation"], "search");
}
