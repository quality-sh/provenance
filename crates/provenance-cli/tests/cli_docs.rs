use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

/// Runs `check --docs --format json` and returns whether it passed and the docs category.
fn check_docs(repo: &std::path::Path) -> (bool, Value) {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--docs",
            "--repo",
            &repo.to_string_lossy(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let categories = report["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 1, "{report}");
    assert_eq!(categories[0]["category"], "docs");
    (output.status.success(), categories[0].clone())
}

// The link decision is checked against generated sites in
// provenance-transport/src/docs/tests.rs. These cases pin it to the command a
// reader runs: the links below are the ones a real docs tree is written with,
// and `check --docs` has to pass or fail on them through the whole
// load-and-resolve path, not just in the decision function.
#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn check_docs_accepts_plain_markdown_tree() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs/guide")).unwrap();
    std::fs::write(
        repo.join("docs/index.md"),
        "# Agent Handbook\n\nSee [Install](guide/install.md).\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("docs/guide/install.md"),
        "# Installation\n\nBack to [home](../index.md).\n",
    )
    .unwrap();

    let (passed, docs) = check_docs(repo);
    assert!(passed, "{docs}");
    assert_eq!(docs["status"], "passed");
    assert_eq!(docs["findings"], serde_json::json!([]));
}

#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn check_docs_fails_on_broken_relative_markdown_links() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(
        repo.join("docs/index.md"),
        "# Agent Handbook\n\nSee [Missing](missing.md).\n",
    )
    .unwrap();

    let (passed, docs) = check_docs(repo);
    assert!(!passed, "{docs}");
    assert_eq!(docs["status"], "findings");
    assert_eq!(docs["findings"].as_array().unwrap().len(), 1);
    let detail = &docs["findings"][0]["detail"];
    assert_eq!(detail["source_path"], "docs/index.md");
    assert_eq!(detail["link"], "missing.md");
}

// `check --docs` reads a page with footnotes on. Read without footnotes, the
// block below is a link definition and `[^1]` the link using it, and the
// command would fail on a page that has no link on it at all.
#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn check_docs_reads_footnotes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(
        repo.join("docs/index.md"),
        "# Agent Handbook\n\nThe handbook is versioned.[^1]\n\n[^1]: guide/missing.md\n",
    )
    .unwrap();

    let (passed, docs) = check_docs(repo);
    assert!(passed, "{docs}");
    assert_eq!(docs["findings"], serde_json::json!([]));
}

// A link out of docs/ and back into the repo root resolves only when the
// site publishes README.md as a page.
#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn check_docs_uses_readme_as_homepage_when_docs_index_is_absent() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs/guide")).unwrap();
    std::fs::write(
        repo.join("README.md"),
        "# Project Handbook\n\nContinue to [Install](docs/guide/install.md).\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("docs/guide/install.md"),
        "# Installation\n\nBack to [home](../../README.md).\n",
    )
    .unwrap();

    let (passed, docs) = check_docs(repo);
    assert!(passed, "{docs}");
    assert_eq!(docs["status"], "passed");
}
