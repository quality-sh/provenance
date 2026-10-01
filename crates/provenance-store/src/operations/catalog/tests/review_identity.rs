#[cfg(feature = "schema")]
#[test]
fn review_writes_do_not_accept_idempotency_headers() {
    for name in [
        "create-requirement",
        "update-requirement",
        "submit-requirement-review",
        "decide-requirement-review",
        "withdraw-requirement-review",
    ] {
        let definition = super::super::definitions()
            .iter()
            .find(|entry| entry.name == name)
            .unwrap();
        let names = definition
            .registration
            .controls
            .headers
            .iter()
            .map(|header| header.name)
            .collect::<Vec<_>>();
        assert!(!names.contains(&"Idempotency-Key"), "{name}: {names:?}");
    }

    let discussions = super::super::definitions()
        .iter()
        .filter(|definition| {
            definition.name.ends_with("-create-discussion")
                || definition.name.ends_with("-update-discussion")
                || definition.name.ends_with("-create-discussion-message")
        })
        .collect::<Vec<_>>();
    assert_eq!(discussions.len(), 18);
    for definition in discussions {
        assert!(
            definition
                .registration
                .controls
                .headers
                .iter()
                .all(|header| header.name != "Idempotency-Key"),
            "{} exposes a client request identity",
            definition.name
        );
    }
}
