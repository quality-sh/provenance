use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

fn prime(repo: &std::path::Path, extra: &[&str]) -> String {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["prime", "--repo", repo.to_str().unwrap()])
        .args(extra)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).unwrap()
}

#[test]
#[verifies("rule_porcelain_cli_prime_teaches_domain", examples)]
fn prime_teaches_domain_without_reading_or_writing_repository_state() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".provenance/state")).unwrap();
    let manifest = repo.path().join(".provenance/state/manifest.json");
    std::fs::write(&manifest, "not a valid manifest").unwrap();

    let text = prime(repo.path(), &[]);
    for term in [
        "Requirement",
        "Rule",
        "Resolution",
        "Implementation binding",
        "Verification",
        "An active Rule can precede its implementation.",
        "A source citation does not count as an Implementation binding.",
        "A graph read does not scan code or establish a coverage verdict.",
    ] {
        assert!(text.contains(term), "guidance omits {term}: {text}");
    }
    let json: Value = serde_json::from_str(&prime(repo.path(), &["--format", "json"])).unwrap();
    assert_eq!(json["guidance"], text.trim_end());
    for field in ["rules", "requirements", "threads", "skills"] {
        assert!(
            json.get(field).is_none(),
            "prime contains project state: {field}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(manifest).unwrap(),
        "not a valid manifest"
    );
    assert_eq!(std::fs::read_dir(repo.path()).unwrap().count(), 1);
    assert_eq!(
        std::fs::read_dir(repo.path().join(".provenance"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        std::fs::read_dir(repo.path().join(".provenance/state"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn legacy_state_options_leave_guidance_unchanged() {
    let repo = tempfile::tempdir().unwrap();
    let text = prime(repo.path(), &[]);
    assert_eq!(
        text,
        prime(repo.path(), &["--scope", "old_scope", "--include-threads"])
    );
    assert_eq!(std::fs::read_dir(repo.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn cli_and_mcp_share_the_same_guidance() {
    use rmcp::ServiceExt as _;

    let repo = tempfile::tempdir().unwrap();
    let text = prime(repo.path(), &[]);
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["init", "--path", repo.path().to_str().unwrap()])
        .assert()
        .success();
    let host =
        provenance_cli::porcelain::local_host(repo.path().to_str().unwrap(), "default").unwrap();
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    assert_eq!(
        client.peer_info().unwrap().instructions.as_deref(),
        Some(text.trim_end())
    );
    let tools = client.list_all_tools().await.unwrap();
    for name in [
        "get",
        "search",
        "create",
        "update",
        "discussion",
        "discuss",
        "reply",
    ] {
        let tool = tools.iter().find(|tool| tool.name == name).unwrap();
        let description = tool.description.as_deref().unwrap();
        assert!(
            text.contains(description),
            "shared description missing: {name}"
        );
    }
    assert!(tools.iter().all(|tool| tool.name != "prime"));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
