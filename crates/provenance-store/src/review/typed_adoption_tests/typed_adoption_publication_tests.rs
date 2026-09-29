use super::*;
use std::process::Command;

fn git(root: &std::path::Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn commit(root: &std::path::Path, message: &str) -> String {
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=Test User",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            message,
        ],
    );
    git(root, &["rev-parse", "HEAD"])
}

#[test]
fn typed_change_stamps_the_enrolled_record_and_journals_the_occurrence() {
    let (temp, store, scope) = fixture();
    git(temp.path(), &["init", "-q"]);
    let commit_a = commit(temp.path(), "Create state");
    store
        .apply_typed_spec(
            &scope,
            document(
                "Policy one",
                "The system stores records.",
                "The system retains records.",
            ),
        )
        .unwrap();
    let requirement = store.list_requirements(&scope).unwrap()[0].clone();
    assert_eq!(requirement.updated.as_ref().unwrap().commit, commit_a);
    enroll(&store, &scope, NodeType::Requirement, &requirement.id);
    std::fs::write(temp.path().join("advance.txt"), "advance\n").unwrap();
    let commit_b = commit(temp.path(), "Advance repository");

    store
        .apply_typed_spec(
            &scope,
            document(
                "Policy one",
                "The system stores durable records.",
                "The system retains records.",
            ),
        )
        .unwrap();

    let changed = store.list_requirements(&scope).unwrap()[0].clone();
    assert_eq!(changed.updated.as_ref().unwrap().commit, commit_b);
    let head = store.head(&changed.into()).unwrap().unwrap();
    assert_eq!(head.sequence, 2);
    assert!(head.changed_fields.iter().any(|field| field == "statement"));
}

#[test]
fn changed_description_of_an_adopted_requirement_gets_a_new_pending_proposal() {
    let (_temp, store, scope) = fixture();
    let mut input = document(
        "Policy one",
        "The system stores records.",
        "The system retains records.",
    );
    store.apply_typed_spec(&scope, input.clone()).unwrap();
    let requirement_id = store.list_requirements(&scope).unwrap()[0].id.clone();
    clear_typed_owner(&store, &scope, NodeType::Requirement, &requirement_id);
    enroll(&store, &scope, NodeType::Requirement, &requirement_id);
    input.requirements[0].id = Some(requirement_id.as_str().to_owned());
    input.adopt_unowned = vec![TypedAdoptionTarget {
        kind: TypedDeclarationKind::Requirement,
        id: requirement_id.as_str().to_owned(),
    }];
    store.apply_typed_spec(&scope, input.clone()).unwrap();

    input.adopt_unowned.clear();
    input.requirements[0].description = Some("First description.".into());
    store.apply_typed_spec(&scope, input.clone()).unwrap();
    let first = store
        .requirement_decision_state(&scope, &requirement_id)
        .unwrap()
        .pending
        .unwrap()
        .proposal_id;

    input.requirements[0].description = Some("Changed description.".into());
    store.apply_typed_spec(&scope, input).unwrap();
    let second = store
        .requirement_decision_state(&scope, &requirement_id)
        .unwrap()
        .pending
        .unwrap()
        .proposal_id;

    assert_ne!(first, second);
}
