use super::*;

#[test]
#[verifies("rule_review_revision_follows_review_content", examples)]
#[verifies("rule_record_history_reads_git", examples)]
fn prerequisite_change_changes_the_review_revision_and_history() {
    let (_dir, repo) = setup();
    create(&repo, "question_a", None);
    create(&repo, "question_b", None);
    for args in [
        vec!["init", "-q"],
        vec!["add", ".provenance/state"],
        vec!["commit", "-q", "-m", "Save the graph"],
    ] {
        let output = std::process::Command::new("git")
            .current_dir(&repo)
            .args([
                "-c",
                "user.name=Reviewer",
                "-c",
                "user.email=reviewer@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
    let before = get(&repo, "questions", "question_b");
    update(
        &repo,
        "questions",
        "question_b",
        json!({"depends_on":["question_a"]}),
    );
    let after = get(&repo, "questions", "question_b");
    assert_ne!(
        after["data"]["edit"]["revision"],
        before["data"]["edit"]["revision"]
    );
    let output = provenance(&["api", "questions/question_b/history", "--repo", &repo])
        .success()
        .get_output()
        .stdout
        .clone();
    let history: Value = serde_json::from_slice(&output).unwrap();
    let working = history["data"]["items"].as_array().unwrap().last().unwrap();
    assert_eq!(working["changed_fields"], json!(["depends_on"]));
    assert_eq!(working["outcome"], "changed");
    update(&repo, "questions", "question_b", json!({"depends_on":[]}));
    assert_eq!(
        get(&repo, "questions", "question_b")["data"]["edit"]["revision"],
        before["data"]["edit"]["revision"]
    );
}
