#[path = "query_support/fixtures.rs"]
mod fixtures;

use provenance_macros::verifies;
use serde_json::{json, Value};

fn settings(repo: &tempfile::TempDir, value: &Value) {
    std::fs::write(
        repo.path().join(".provenance/settings.json"),
        serde_json::to_vec(value).unwrap(),
    )
    .unwrap();
}

fn request() -> Value {
    json!({"node_type": "requirement", "id": "req_missing"})
}

#[test]
#[verifies("rule_invalid_read_setting_is_a_typed_refusal", examples)]
fn a_settings_refusal_produces_no_answer_and_no_freshness_error() {
    let repo = fixtures::init_repo();
    settings(&repo, &json!({"read": {"freshness_policy": "fast"}}));
    let (ok, stdout, stderr) = fixtures::sdk_raw(repo.path().to_str().unwrap(), "get", &request());
    assert!(!ok, "invalid settings produced an answer: {stdout}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("read.freshness_policy"), "{stderr}");
    assert!(!stderr.contains("freshness_error"), "{stderr}");
    assert!(!repo.path().join(".provenance/cache/provenance.db").exists());
}

#[test]
#[verifies("rule_freshness_flag_wins_over_the_settings_file", examples)]
fn the_freshness_flag_wins_over_the_settings_file() {
    let repo = fixtures::init_repo();
    let path = repo.path().to_str().unwrap();
    settings(
        &repo,
        &json!({"read": {"freshness_policy": "annotate_only"}}),
    );
    let error = fixtures::sdk_error(path, "get", &request());
    assert!(error.contains("provenance materialize"), "{error}");
    let output = fixtures::provenance()
        .args(["sdk", "get", "--repo", path, "--freshness", "catch_up"])
        .write_stdin(request().to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(answer["stamp"]["policy"], "catch_up");
    let from_file = fixtures::sdk(path, "get", &request());
    assert_eq!(from_file["stamp"]["policy"], "annotate_only");
    settings(&repo, &json!({"read": {"freshness_policy": "catch_up"}}));
    assert_eq!(
        fixtures::sdk(path, "get", &request())["stamp"]["policy"],
        "catch_up"
    );
}

#[test]
#[verifies("rule_freshness_flag_wins_over_the_settings_file", examples)]
fn the_stamp_names_the_policy_the_flag_chose() {
    let repo = fixtures::init_repo();
    let path = repo.path().to_str().unwrap();
    std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(path)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "--allow-empty",
            "--quiet",
            "-m",
            "Initialize test repository",
        ])
        .current_dir(path)
        .status()
        .unwrap();
    fixtures::sdk(path, "get", &request());
    for command in [
        "get",
        "search",
        "neighbors",
        "trace",
        "impact",
        "evidence",
        "stale",
        "resolve-symbol",
    ] {
        let input = match command {
            "get" => request(),
            "search" => json!({"text": "missing"}),
            "evidence" => json!({"rule": "rule_missing"}),
            "stale" => json!({"base": "HEAD"}),
            "resolve-symbol" => json!({"file": "missing.rs"}),
            _ => json!({"id": "req_missing"}),
        };
        let output = fixtures::provenance()
            .args([
                "sdk",
                command,
                "--repo",
                path,
                "--freshness",
                "annotate_only",
            ])
            .write_stdin(input.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(answer["stamp"]["policy"], "annotate_only", "{command}");
    }
}

#[test]
fn the_settings_scan_limit_bounds_impact_and_changes_no_revision() {
    let repo = fixtures::init_repo();
    let path = repo.path().to_str().unwrap();
    for name in ["a.rs", "b.rs"] {
        std::fs::write(repo.path().join(name), "fn example() {}\n").unwrap();
    }
    let input = json!({"id": "req_missing"});
    let before = fixtures::sdk(path, "impact", &input);
    assert_eq!(before["scan_cut"], false);
    settings(&repo, &json!({"read": {"scan_limit": 1}}));
    let after = fixtures::sdk(path, "impact", &input);
    assert_eq!(after["scan_cut"], true);
    assert_eq!(after["stamp"], before["stamp"]);
}

#[test]
fn a_flag_cannot_hide_invalid_settings() {
    let repo = fixtures::init_repo();
    settings(&repo, &json!({"read": {"freshness_policy": "fast"}}));
    let output = fixtures::provenance()
        .args([
            "sdk",
            "get",
            "--repo",
            repo.path().to_str().unwrap(),
            "--freshness",
            "catch_up",
        ])
        .write_stdin(request().to_string())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("read.freshness_policy"));
    assert!(!repo.path().join(".provenance/cache/provenance.db").exists());
}

#[test]
fn refuse_stale_reaches_the_reserved_policy_and_bad_flag_words_refuse() {
    let repo = fixtures::init_repo();
    let path = repo.path().to_str().unwrap();
    let output = fixtures::provenance()
        .args(["sdk", "get", "--repo", path, "--freshness", "refuse_stale"])
        .write_stdin(request().to_string())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("reserved and not implemented"));
    for word in ["fast", "catch_up_failed"] {
        let output = fixtures::provenance()
            .args(["sdk", "get", "--repo", path, "--freshness", word])
            .write_stdin(request().to_string())
            .output()
            .unwrap();
        assert!(!output.status.success());
        let text = String::from_utf8_lossy(&output.stderr);
        for allowed in ["catch_up", "annotate_only", "refuse_stale"] {
            assert!(text.contains(allowed), "{text}");
        }
    }
}
