//! A write of an active Rule or an answered Question reads the citations in
//! order. A Resolution whose review rows disagree refuses at its place in
//! that order, with the refusal of a read of that Resolution.

use super::review_cycle::write;
use super::review_disagreeing_rows::refused;
use super::review_merged_withdrawals::{read, repository, Merge};
use super::review_row_dependence::{create_resolution, decide_resolution_on_two_branches, patch};
use super::*;

/// The refusal of a citation of the draft Resolution `id` by the Rule `rule`.
fn not_approved(rule: &str, id: &str) -> Value {
    json!({"kind":"resolution_not_approved", "resolution_id":id,
        "message":format!("rule {rule} cites resolution {id}, which is draft. An answered \
            Question or an active Rule can cite only an approved Resolution. Until a reviewer \
            approves {id}, keep the Rule in draft or review, or leave the Question open, or \
            remove the citation and name {id} in the text.")})
}

/// The fields of `error` that `expected` names.
fn fields_of(error: &Value, expected: &Value) -> Value {
    let fields = expected.as_object().unwrap().keys();
    fields
        .map(|field| (field.clone(), error[field].clone()))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// Flow: the draft Resolutions `res_a` and `res_c` have no approval, and
/// the review rows of `res_b` disagree. A Rule keeps its citations sorted by
/// ID. The draft Rule `rule_a` cites `res_a` and `res_b`, and `rule_c` cites
/// `res_b` and `res_c`. The change of each Rule to active refuses for its
/// first citation that is not approved.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
fn a_change_to_active_refuses_for_the_first_citation_that_is_not_approved() {
    let (_directory, repo) = repository(Merge::Driver);
    for id in ["res_a", "res_b", "res_c"] {
        create_resolution(&repo, id);
    }
    for (rule, cited) in [
        ("rule_a", ["res_b", "res_a"]),
        ("rule_c", ["res_c", "res_b"]),
    ] {
        write(
            &repo,
            "post",
            "rules",
            &json!({"id":rule, "statement":"The system keeps one copy.", "severity":"medium",
                "status":"draft", "requirement_ids":["req_a"], "resolution_ids":cited}),
            None,
        );
    }
    decide_resolution_on_two_branches(&repo, "res_b");
    let fault = refused(&repo, &["api", "resolutions/res_b"]);
    assert_eq!(fault["kind"], "invalid_state_file", "{fault}");

    let active = |rule: &str| {
        let path = format!("rules/{rule}");
        let cited = read(&repo, &["api", &path])["data"]["resolution_ids"].clone();
        let arguments = patch(&repo, &path, &json!({"status":"active"}));
        let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
        (cited, refusal(&arguments)["error"].clone())
    };
    let expected = not_approved("rule_a", "res_a");
    let (cited, error) = active("rule_a");
    assert_eq!(cited, json!(["res_a", "res_b"]));
    assert_eq!(fields_of(&error, &expected), expected);
    let (cited, error) = active("rule_c");
    assert_eq!(cited, json!(["res_b", "res_c"]));
    assert_eq!(error, fault);
    for rule in ["rule_a", "rule_c"] {
        let path = format!("rules/{rule}");
        assert_eq!(read(&repo, &["api", &path])["data"]["status"], "draft");
    }
}

/// Flow: the open Question `question_a` waits for an answer, and the review
/// rows of the Resolution `res_a` disagree. The answer that cites `res_a`
/// gives the refusal of `res_a`, and the Question stays open.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
fn an_answer_that_cites_a_resolution_whose_rows_disagree_gives_its_refusal() {
    let (_directory, repo) = repository(Merge::Driver);
    create_resolution(&repo, "res_a");
    write(
        &repo,
        "post",
        "topics",
        &json!({"id":"topic_copy", "title":"Copies", "requirement_id":"req_a",
            "status":"open", "links":[]}),
        None,
    );
    write(
        &repo,
        "post",
        "questions",
        &json!({"id":"question_a", "topic_id":"topic_copy",
            "question":"Which threshold applies?", "resolution_method":"research",
            "status":"open", "links":[]}),
        None,
    );
    decide_resolution_on_two_branches(&repo, "res_a");
    let fault = refused(&repo, &["api", "resolutions/res_a"]);
    assert_eq!(fault["kind"], "invalid_state_file", "{fault}");

    let answer = json!({"answer":"Yes. The system keeps one copy.", "resolution_id":"res_a"});
    let input = directory_file(&repo, "answer.json", &answer.to_string());
    let arguments = [
        "api",
        "questions/question_a/answer",
        "--method",
        "post",
        "--input",
        &input,
    ];
    assert_eq!(refused(&repo, &arguments), fault);
    let question = read(&repo, &["api", "questions/question_a"]);
    assert_eq!(question["data"]["status"], "open", "{question}");
}
