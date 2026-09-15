#[test]
fn catalog_address_uses_resource_paths() {
    let (method, path, _) =
        super::address("topics", vec!["topic_a".into(), "claim".into()]).unwrap();
    assert_eq!(method, axum::http::Method::POST);
    assert_eq!(path, "/topics/topic_a/claim");
}

#[test]
fn catalog_address_uses_parent_owned_paths() {
    let (_, review, _) = super::address(
        "requirements",
        vec![
            "req_a".into(),
            "submissions".into(),
            "proposal_a".into(),
            "decide".into(),
        ],
    )
    .unwrap();
    let (_, evidence, _) = super::address(
        "requirements",
        vec![
            "req_a".into(),
            "history".into(),
            "entry_a".into(),
            "evidence".into(),
            "before".into(),
        ],
    )
    .unwrap();
    let (method, message, _) = super::address(
        "sources",
        vec![
            "source_a".into(),
            "discussions".into(),
            "discussion_a".into(),
            "messages".into(),
            "create".into(),
        ],
    )
    .unwrap();
    assert_eq!(review, "/requirements/req_a/submissions/proposal_a/decide");
    assert_eq!(
        evidence,
        "/requirements/req_a/history/entry_a/evidence/before"
    );
    assert_eq!(method, axum::http::Method::POST);
    assert_eq!(
        message,
        "/sources/source_a/discussions/discussion_a/messages"
    );
}
