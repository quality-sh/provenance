use assert_cmd::Command;
use provenance_macros::verifies;
use std::path::Path;

#[test]
#[verifies("rule_init_guidance_selected_scope", examples)]
fn fresh_init_names_the_explicit_scope() {
    let temporary = tempfile::tempdir().unwrap();
    init(temporary.path(), &["--scope", "demo"]);
    assert_scope(temporary.path(), "demo");
}

#[test]
#[verifies("rule_init_guidance_selected_scope", examples)]
fn reinit_without_scope_keeps_the_single_manifest_scope() {
    let temporary = tempfile::tempdir().unwrap();
    init(temporary.path(), &["--scope", "demo"]);
    // A corrected managed section must survive initialization without --scope.
    let agents = temporary.path().join("AGENTS.md");
    let corrected = std::fs::read_to_string(&agents)
        .unwrap()
        .replace("--scope default", "--scope demo");
    std::fs::write(&agents, &corrected).unwrap();
    init(temporary.path(), &[]);
    assert_scope(temporary.path(), "demo");
    assert_eq!(std::fs::read_to_string(agents).unwrap(), corrected);
}

#[test]
#[verifies("rule_init_guidance_selected_scope", examples)]
fn fresh_init_without_scope_uses_default() {
    let temporary = tempfile::tempdir().unwrap();
    init(temporary.path(), &[]);
    assert_scope(temporary.path(), "default");
}

#[test]
#[verifies("rule_init_guidance_selected_scope", examples)]
fn multiple_scopes_without_selection_keep_default_guidance() {
    let temporary = tempfile::tempdir().unwrap();
    init(temporary.path(), &["--scope", "demo"]);
    init(temporary.path(), &["--scope", "other"]);
    assert_scope(temporary.path(), "other");
    init(temporary.path(), &[]);
    assert_scope(temporary.path(), "default");
}

fn init(repo: &Path, extra: &[&str]) {
    Command::cargo_bin("provenance")
        .unwrap()
        .arg("init")
        .arg("--path")
        .arg(repo)
        .args(extra)
        .assert()
        .success();
}

fn assert_scope(repo: &Path, expected: &str) {
    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    let scopes: Vec<_> = agents
        .split("--scope ")
        .skip(1)
        .map(|tail| tail.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(scopes, vec![expected; 5], "{agents}");
}
