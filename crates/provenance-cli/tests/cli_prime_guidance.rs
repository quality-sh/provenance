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
    std::fs::create_dir(repo.path().join(".provenance")).unwrap();
    let manifest = repo.path().join(".provenance/manifest.toml");
    std::fs::write(&manifest, "not a valid manifest").unwrap();

    let text = prime(repo.path(), &[]);
    for term in ["Requirement", "Rule", "Resolution", "Implementation binding", "Verification"] {
        assert!(text.contains(term), "guidance omits {term}: {text}");
    }
    let json: Value = serde_json::from_str(&prime(repo.path(), &["--format", "json"])).unwrap();
    assert_eq!(json["guidance"], text.trim_end());
    for field in ["rules", "requirements", "threads", "skills"] {
        assert!(json.get(field).is_none(), "prime contains project state: {field}");
    }
    assert_eq!(std::fs::read_to_string(manifest).unwrap(), "not a valid manifest");
    assert_eq!(std::fs::read_dir(repo.path()).unwrap().count(), 1);
    assert_eq!(std::fs::read_dir(repo.path().join(".provenance")).unwrap().count(), 1);
}

#[test]
fn legacy_state_options_leave_guidance_unchanged() {
    let repo = tempfile::tempdir().unwrap();
    let text = prime(repo.path(), &[]);
    assert_eq!(text, prime(repo.path(), &["--scope", "old_scope", "--include-threads"]));
    assert_eq!(std::fs::read_dir(repo.path()).unwrap().count(), 0);
}
