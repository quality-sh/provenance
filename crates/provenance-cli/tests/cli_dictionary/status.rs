use crate::support::{dictionary_pdf, init, provenance, reference_path, write_reference};
use serde_json::Value;

#[test]
fn status_reports_the_referenced_identity_and_a_present_index_after_an_import() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    let pdf = scratch.path().join("dictionary.pdf");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(&pdf, dictionary_pdf()).unwrap();
    init(&repo);
    provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .args([
            "dictionary",
            "import",
            "--pdf",
            pdf.to_str().unwrap(),
            "--repo",
            repo.to_str().unwrap(),
        ])
        .assert()
        .success();

    let output = status(&repo, &index_directory);

    assert!(
        output.status.success(),
        "the status must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["referenced"], true);
    assert_eq!(status["issue"], 9);
    assert_eq!(status["source_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(status["data_sha256"].as_str().unwrap().len(), 64);
    assert!(!status["extractor_version"].as_str().unwrap().is_empty());
    assert_eq!(status["index_present"], true);
    assert_eq!(status["analyzer"], "project_dictionary");
    assert!(status["load_problem"].is_null());
    assert_eq!(
        status["index_directory"],
        index_directory.display().to_string()
    );
}

#[test]
fn status_reports_a_missing_reference_as_built_in_rules() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&index_directory).unwrap();
    init(&repo);

    let output = status(&repo, &index_directory);

    assert!(output.status.success());
    let status: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["referenced"], false);
    assert!(status["issue"].is_null());
    assert_eq!(status["index_present"], false);
    assert_eq!(status["analyzer"], "built_in_rules");
    assert!(status["load_problem"].is_null());
    assert!(!reference_path(&repo).exists());
}

#[test]
fn status_reports_a_reference_without_an_index_as_built_in_rules() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("empty-index");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&index_directory).unwrap();
    init(&repo);
    write_reference(&repo, crate::support::imported_dictionary());

    let output = status(&repo, &index_directory);

    assert!(output.status.success());
    let status: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["referenced"], true);
    assert_eq!(status["issue"], 9);
    assert_eq!(status["index_present"], false);
    assert_eq!(status["analyzer"], "built_in_rules");
    let problem = status["load_problem"].as_str().unwrap();
    assert!(problem.contains("no index file"), "problem: {problem}");
}

fn status(repo: &std::path::Path, index_directory: &std::path::Path) -> std::process::Output {
    provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", index_directory)
        .args([
            "dictionary",
            "status",
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap()
}
