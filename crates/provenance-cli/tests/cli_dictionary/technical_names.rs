use crate::support::{
    create_requirement, dictionary_pdf, error_json, imported_dictionary, init, provenance,
    reference_path, write_reference_with_technical_names, UNAPPROVED_WORD,
};
use provenance_macros::verifies;
use provenance_ste100::store_dictionary_index;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Two more unapproved headwords from the synthetic fixture.
const SECOND_UNAPPROVED_WORD: &str = "bunapprovedaab";
const THIRD_UNAPPROVED_WORD: &str = "bunapprovedaac";

fn setup(technical_names: &[&str]) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    init(&repo);
    let dictionary = imported_dictionary();
    store_dictionary_index(dictionary, &index_directory).unwrap();
    write_reference_with_technical_names(&repo, dictionary, technical_names);
    (scratch, repo, index_directory)
}

fn rejected_words(statement: &str, refusal: &Value) -> Vec<String> {
    refusal["report"]["findings"]
        .as_array()
        .expect("the refusal lists findings")
        .iter()
        .filter(|finding| finding["rule"] == "1.1")
        .map(|finding| {
            let start = usize::try_from(finding["span"]["start"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(finding["span"]["end"].as_u64().unwrap()).unwrap();
            statement[start..end].to_owned()
        })
        .collect()
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
fn a_write_with_a_listed_technical_name_passes() {
    let (_scratch, repo, index_directory) = setup(&[UNAPPROVED_WORD]);
    let statement = format!("The {UNAPPROVED_WORD} item stops.");

    let output = create_requirement(&repo, &index_directory, "req_listed", &statement);

    assert!(
        output.status.success(),
        "Rule 1.1 must accept a listed technical name: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
fn a_listed_technical_name_matches_with_other_case_and_a_possessive() {
    let (_scratch, repo, index_directory) = setup(&[UNAPPROVED_WORD]);
    let statement = format!("The {}'s item stops.", UNAPPROVED_WORD.to_uppercase());

    let output = create_requirement(&repo, &index_directory, "req_folded", &statement);

    assert!(
        output.status.success(),
        "the match must fold case and a possessive: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
fn a_listed_technical_name_of_more_than_one_word_passes() {
    let phrase = format!("{UNAPPROVED_WORD} item");
    let (_scratch, repo, index_directory) = setup(&[&phrase]);

    for (id, statement) in [
        ("req_phrase", format!("The {UNAPPROVED_WORD} item stops.")),
        (
            "req_phrase_folded",
            format!(
                "The {}  Item's cover stops.",
                UNAPPROVED_WORD.to_uppercase()
            ),
        ),
    ] {
        let output = create_requirement(&repo, &index_directory, id, &statement);

        assert!(
            output.status.success(),
            "Rule 1.1 must accept the listed phrase in {statement:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
fn one_word_of_a_listed_phrase_alone_still_fails() {
    let phrase = format!("{UNAPPROVED_WORD} item");
    let (_scratch, repo, index_directory) = setup(&[&phrase]);
    let statement = format!("The {UNAPPROVED_WORD} stops.");

    let output = create_requirement(&repo, &index_directory, "req_part", &statement);

    assert!(!output.status.success(), "the write must fail");
    assert_eq!(
        rejected_words(&statement, &error_json(&output)),
        [UNAPPROVED_WORD]
    );
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
fn the_same_write_with_an_unlisted_unapproved_word_fails_and_names_the_word() {
    let (_scratch, repo, index_directory) = setup(&[UNAPPROVED_WORD]);
    let statement = format!("The {UNAPPROVED_WORD} item and the {SECOND_UNAPPROVED_WORD} stop.");

    let output = create_requirement(&repo, &index_directory, "req_unlisted", &statement);

    assert!(!output.status.success(), "the write must fail");
    let refusal = error_json(&output);
    assert_eq!(refusal["kind"], "statement_invalid", "{refusal}");
    assert_eq!(
        rejected_words(&statement, &refusal),
        [SECOND_UNAPPROVED_WORD]
    );
    let message = refusal["report"]["findings"][0]["message"]
        .as_str()
        .unwrap();
    assert!(
        message.contains(SECOND_UNAPPROVED_WORD),
        "the finding must name the word: {message}"
    );
}

#[test]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
fn a_refused_write_names_all_its_unapproved_words() {
    let (_scratch, repo, index_directory) = setup(&[]);
    let statement = format!(
        "The {UNAPPROVED_WORD} item, the {SECOND_UNAPPROVED_WORD} item and the \
         {THIRD_UNAPPROVED_WORD} item stop."
    );

    let output = create_requirement(&repo, &index_directory, "req_all", &statement);

    assert!(!output.status.success(), "the write must fail");
    let refusal = error_json(&output);
    assert_eq!(
        rejected_words(&statement, &refusal),
        [
            UNAPPROVED_WORD,
            SECOND_UNAPPROVED_WORD,
            THIRD_UNAPPROVED_WORD
        ]
    );
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
fn a_dictionary_import_keeps_the_technical_names_of_the_reference() {
    let (scratch, repo, index_directory) = setup(&[UNAPPROVED_WORD]);
    let pdf = scratch.path().join("dictionary.pdf");
    std::fs::write(&pdf, dictionary_pdf()).unwrap();

    import(&repo, &index_directory, &pdf);

    let reference: Value =
        serde_json::from_slice(&std::fs::read(reference_path(&repo)).unwrap()).unwrap();
    assert_eq!(
        reference["technical_names"],
        serde_json::json!([UNAPPROVED_WORD])
    );
    let statement = format!("The {UNAPPROVED_WORD} item stops.");
    let output = create_requirement(&repo, &index_directory, "req_after_import", &statement);
    assert!(
        output.status.success(),
        "the names must still apply after an import: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn import(repo: &Path, index_directory: &Path, pdf: &Path) {
    provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", index_directory)
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
}
