use super::{state_view, unit_digest, unit_digests, unit_stored_digest, Unit, UnitDigests};

#[test]
fn an_entry_that_is_not_canonical_state_moves_no_digest() {
    for unit in [
        Unit::Global,
        Unit::Scope(provenance_core::ScopeId::new("default").unwrap()),
    ] {
        let (dir, _) = requirement_shard(b"{\"id\":\"one\"}\n");
        let state = camino::Utf8Path::from_path(dir.path()).unwrap();
        let before = unit_digest(state, &unit).unwrap();
        let root = match &unit {
            Unit::Global => state.to_path_buf(),
            Unit::Scope(scope) => state.join("scopes").join(scope.as_str()),
        };
        std::fs::write(root.join("added.txt"), b"added").unwrap();
        std::fs::write(root.join(".tmp123"), b"half-written").unwrap();
        assert_eq!(before, unit_digest(state, &unit).unwrap());
    }
}

#[test]
fn stamps_do_not_change_unit_digests_but_every_other_field_does() {
    let dir = tempfile::tempdir().unwrap();
    let state = camino::Utf8Path::from_path(dir.path()).unwrap();
    let unit = Unit::Scope(provenance_core::ScopeId::new("default").unwrap());
    for family in ["sources", "requirements", "rules", "resolutions"] {
        let shard = match family {
            "sources" => "source",
            "requirements" => "req",
            "rules" => "rule",
            _ => "res",
        };
        let path = state.join(format!("scopes/default/{family}/{shard}.jsonl"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut record = serde_json::json!({"id":"one","statement":"Original content"});
        let write = |record: &serde_json::Value| {
            std::fs::write(&path, serde_json::to_vec(record).unwrap()).unwrap();
        };
        write(&record);
        let before = unit_digest(state, &unit).unwrap();
        record["created"] =
            serde_json::json!({"commit":"a".repeat(40),"at":"2026-09-12T00:00:00Z"});
        record["updated"] =
            serde_json::json!({"commit":"b".repeat(64),"at":"2026-09-12T01:00:00Z"});
        write(&record);
        assert_eq!(before, unit_digest(state, &unit).unwrap(), "{family}");
        record["statement"] = serde_json::json!("Hand edited content");
        write(&record);
        assert_ne!(before, unit_digest(state, &unit).unwrap(), "{family}");
        let before_archive = unit_digest(state, &unit).unwrap();
        record["archived_in_commit"] = serde_json::json!({"commit":"c".repeat(40)});
        write(&record);
        assert_ne!(
            before_archive,
            unit_digest(state, &unit).unwrap(),
            "archive permalink is content"
        );
    }
}

fn requirement_shard(bytes: &[u8]) -> (tempfile::TempDir, Unit) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scopes/default/requirements/req.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    (
        dir,
        Unit::Scope(provenance_core::ScopeId::new("default").unwrap()),
    )
}

#[test]
fn a_known_pair_with_the_current_stored_digest_is_returned_without_a_parse() {
    // The shard is not JSON, so any parse of it fails.
    let (dir, unit) = requirement_shard(b"not json\n");
    let state = camino::Utf8Path::from_path(dir.path()).unwrap();
    let known = UnitDigests {
        content: "fake content digest".to_string(),
        stored: unit_stored_digest(state, &unit).unwrap(),
    };

    let digests = unit_digests(state, &state_view(state).unwrap(), &unit, Some(&known)).unwrap();

    assert_eq!(digests, known);
}

#[test]
fn a_known_pair_with_a_different_stored_digest_gets_the_computed_content_digest() {
    let (dir, unit) = requirement_shard(br#"{"id":"one","statement":"Original content"}"#);
    let state = camino::Utf8Path::from_path(dir.path()).unwrap();
    let known = UnitDigests {
        content: "fake content digest".to_string(),
        stored: "fake stored digest".to_string(),
    };

    let digests = unit_digests(state, &state_view(state).unwrap(), &unit, Some(&known)).unwrap();

    assert_eq!(digests.content, unit_digest(state, &unit).unwrap());
    assert_eq!(digests.stored, unit_stored_digest(state, &unit).unwrap());
}
