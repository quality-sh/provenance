use super::*;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use serde_json::json;

// Implementation aid: check the public export contract for the new record field.
#[test]
fn exported_question_prerequisites_match_the_public_schema() {
    let temp = committed_store();
    let stamp = SUPPORTED_SCHEMA_VERSION.0;
    let seed = json!({"scope":"default", "sources":[], "resolutions":[], "rules":[],
        "requirements":[{"schema_version":stamp,"scope_id":"default","id":"req_pay",
            "statement":"The system calculates overtime pay.","status":"discovery"}],
        "topics":[{"schema_version":stamp,"scope_id":"default","id":"topic_pay",
            "requirement_id":"req_pay","title":"Overtime pay","status":"open","links":[]}],
        "questions":[
            {"schema_version":stamp,"scope_id":"default","id":"question_threshold",
                "topic_id":"topic_pay","requirement_id":"req_pay","question":"Which threshold applies?",
                "resolution_method":"research","status":"open","links":[],"depends_on":[]},
            {"schema_version":stamp,"scope_id":"default","id":"question_rate",
                "topic_id":"topic_pay","requirement_id":"req_pay","question":"Which rate applies?",
                "resolution_method":"grill","status":"open","links":[],"depends_on":["question_threshold"]}
        ]});
    let path = temp.path().join("seed.json");
    std::fs::write(&path, seed.to_string()).unwrap();
    provenance(temp.path()).args(["import","--repo",".","--scope","default","--input",
        path.to_str().unwrap()]).assert().success();
    let status = std::process::Command::new("git").current_dir(temp.path())
        .args(["add", ".provenance/state"]).status().unwrap();
    assert!(status.success());
    let status = std::process::Command::new("git").current_dir(temp.path())
        .args(["commit", "-qm", "Save the prerequisite graph"]).status().unwrap();
    assert!(status.success());
    let reference = issue(temp.path(), &[]);
    let reference_path = write_reference(temp.path(), &reference);
    let output = provenance(temp.path()).args(["graph-reference","exact-export","--repo",".",
        "--reference",reference_path.to_str().unwrap()]).assert().success()
        .get_output().stdout.clone();
    let document: Value = serde_json::from_slice(&output).unwrap();
    let question = document["graph"]["questions"].as_array().unwrap().iter()
        .find(|row| row["id"] == "question_rate").unwrap();
    assert_eq!(question["depends_on"], json!(["question_threshold"]));
    let output = provenance(temp.path()).args(["schema","show","graph-reference-export",
        "--format","json"]).assert().success().get_output().stdout.clone();
    let shown: Value = serde_json::from_slice(&output).unwrap();
    let validator = jsonschema::JSONSchema::compile(&shown["schema"]).unwrap();
    let errors: Vec<_> = validator.validate(&document).err().into_iter()
        .flatten().map(|error| error.to_string()).collect();
    assert!(errors.is_empty(), "export schema refused the document: {errors:?}");
}
