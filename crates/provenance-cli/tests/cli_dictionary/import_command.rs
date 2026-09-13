use crate::support::{
    create_requirement, dictionary_pdf, init, provenance, reference_path, APPROVED_TABLE_ROWS,
    UNAPPROVED_TABLE_ROWS, UNAPPROVED_WORD,
};
use provenance_macros::verifies;
use provenance_ste100::DictionaryImportIdentity;
use serde_json::Value;

#[test]
#[verifies("rule_ste_dictionary_reference_resolution", examples)]
fn the_import_command_stores_an_index_that_later_checks_use() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    let pdf = scratch.path().join("dictionary.pdf");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(&pdf, dictionary_pdf()).unwrap();
    init(&repo);

    let output = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .args([
            "dictionary",
            "import",
            "--pdf",
            pdf.to_str().unwrap(),
            "--repo",
            repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "the import must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["issue"], 9);
    assert_eq!(summary["source_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(summary["data_sha256"].as_str().unwrap().len(), 64);
    assert!(!summary["extractor_version"].as_str().unwrap().is_empty());
    assert_eq!(summary["approved_rows"], APPROVED_TABLE_ROWS);
    assert_eq!(summary["unapproved_rows"], UNAPPROVED_TABLE_ROWS);
    assert_eq!(
        summary.as_object().unwrap().keys().collect::<Vec<_>>(),
        [
            "approved_rows",
            "data_sha256",
            "extractor_version",
            "issue",
            "source_sha256",
            "unapproved_rows"
        ],
        "the summary must hold no dictionary content"
    );

    let reference: DictionaryImportIdentity =
        serde_json::from_slice(&std::fs::read(reference_path(&repo)).unwrap()).unwrap();
    assert_eq!(reference.source_sha256, summary["source_sha256"]);
    assert_eq!(reference.data_sha256, summary["data_sha256"]);

    let write = create_requirement(
        &repo,
        &index_directory,
        "req_after_import",
        &format!("The {UNAPPROVED_WORD} item stops."),
    );
    assert!(
        !write.status.success(),
        "the stored index must reject an unapproved word"
    );
}

#[test]
fn the_import_command_accepts_a_relative_pdf_path() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(scratch.path().join("dictionary.pdf"), dictionary_pdf()).unwrap();
    init(&repo);

    let output = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .current_dir(scratch.path())
        .args([
            "dictionary",
            "import",
            "--pdf",
            "dictionary.pdf",
            "--repo",
            repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "a relative --pdf path must resolve against the working directory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(reference_path(&repo).exists());
}

#[test]
fn the_import_command_rejects_input_that_is_not_a_pdf() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    let pdf = scratch.path().join("not-a-dictionary.pdf");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(&pdf, b"not a PDF").unwrap();
    init(&repo);

    let output = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .args([
            "dictionary",
            "import",
            "--pdf",
            pdf.to_str().unwrap(),
            "--repo",
            repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "invalid input must fail");
    assert!(
        !reference_path(&repo).exists(),
        "a failed import must write no reference"
    );
}
