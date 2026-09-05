use super::*;
use provenance_macros::verifies;

#[test]
#[verifies("rule_page_route_safety", examples)]
fn rejects_a_route_that_would_escape_the_stage() {
    let temp = tempfile::tempdir().unwrap();
    let stage = utf8(temp.path().join("stage"));
    std::fs::create_dir(&stage).unwrap();

    let error = write_page(&stage, "/../escaped/", "caller overwrite").unwrap_err();

    assert!(matches!(error, PublishError::InvalidRoute { .. }));
    assert!(!utf8(temp.path().join("escaped/index.html")).exists());
}

#[test]
#[verifies("rule_page_route_safety", examples)]
fn staging_failure_preserves_the_previous_complete_output() {
    let temp = tempfile::tempdir().unwrap();
    let output = utf8(temp.path().join("wiki"));
    publish(&empty_corpus(), PublicationOutput::custom(output.clone())).unwrap();
    let previous_index = std::fs::read(output.join("index.html")).unwrap();
    let mut invalid = empty_corpus();
    invalid.requirements.push(RequirementPage {
        id: PageId::new(RecordKind::Requirement, "../escape"),
        title: "unsafe".to_string(),
        status: RequirementStatus::Active,
        statement: "unsafe".to_string(),
        description: None,
        fog: None,
        domain_id: None,
        domain_has_anchor: false,
        back_link: None,
        lineage: Vec::new(),
        decisions: Vec::new(),
        produced_rules: Vec::new(),
        children: Vec::new(),
        siblings: Vec::new(),
        supersedes: Vec::new(),
        depends_on: Vec::new(),
        superseded_by: None,
        sources: Vec::new(),
        gaps: Vec::new(),
        threads: Vec::new(),
    });

    let error = publish(&invalid, PublicationOutput::custom(output.clone())).unwrap_err();

    assert!(matches!(error, PublishError::InvalidRoute { .. }));
    assert_eq!(
        std::fs::read(output.join("index.html")).unwrap(),
        previous_index
    );
    assert!(!artifact(&output, "stage").exists());
    assert!(!artifact(&output, "stage.cleanup").exists());
    assert!(!utf8(temp.path().join("escape")).exists());

    publish(&empty_corpus(), PublicationOutput::custom(output.clone())).unwrap();
    assert_no_transaction_artifacts(&output);
}
