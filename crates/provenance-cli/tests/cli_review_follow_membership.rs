#[path = "review_host_support/mod.rs"]
mod review_host_support;
use provenance_store::layout::ProvenanceLayout;
use review_host_support::{
    follow::{create, Follow},
    repository, start, waits,
};

fn remove_shard(root: &std::path::Path, shard: &str) {
    let layout =
        ProvenanceLayout::new(camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap());
    waits::publish(&layout, || {
        let target = root.join(format!(".provenance/state/scopes/default/{shard}"));
        assert!(target.exists(), "canonical shard must exist: {}", target.display());
        let replacement = target.with_extension("replacement");
        std::fs::write(&replacement, "")?;
        std::fs::rename(replacement, target)?;
        Ok(())
    })
    .unwrap();
}

#[test]
// Implementation aid: external removal must identify exactly the record absent from the document.
fn removed_document_member_is_reported_in_removed_ids() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    create(
        repo.path(),
        "rule_follow",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    assert!(follow.frame()["records"]
        .to_string()
        .contains("rule_follow"));
    remove_shard(repo.path(), "rules/rule.jsonl");
    let changed = follow.frame();
    assert_eq!(changed["type"], "state");
    assert_eq!(changed["continuity"], "current");
    assert_eq!(changed["removedIds"], serde_json::json!(["rule_follow"]));
    assert!(!changed["records"].to_string().contains("rule_follow"));
}

#[test]
// Implementation aid: deleting the root is a read failure while the host remains available.
fn deleted_root_reports_a_read_error_with_its_reason() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let mut host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    follow.frame();
    remove_shard(repo.path(), "requirements/req.jsonl");
    let error = follow.frame();
    assert_eq!(error["type"], "read-error");
    assert_eq!(error["continuity"], "unknown");
    assert!(error["reason"].as_str().unwrap().contains("req_follow"));
    assert!(host.is_running());
}
