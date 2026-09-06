use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_store::{cache::scope_ids, layout::ProvenanceLayout};

#[tokio::test]
async fn scope_ids_read_the_manifest_while_the_publication_guard_is_held() {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest =
        Manifest::default_with_scope(ScopeId::new("default").unwrap(), RepoPathPrefix::new("."));
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let guard = provenance_store::publication::publication_guard(&layout)
        .await
        .unwrap();
    let state = layout.state_dir();
    let (sender, receiver) = std::sync::mpsc::channel();
    let read = std::thread::spawn(move || {
        sender.send(scope_ids(&state)).unwrap();
    });
    let result = receiver.recv_timeout(std::time::Duration::from_secs(2));
    drop(guard);
    read.join().unwrap();
    assert_eq!(
        result
            .expect("scope listing must not wait for the guard")
            .unwrap(),
        vec![manifest.scopes[0].id.clone()]
    );
}

#[test]
fn scope_ids_refuse_an_unsupported_or_malformed_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let path = camino::Utf8Path::from_path(dir.path()).unwrap();
    for bytes in [r#"{"schema_version":999,"scopes":[]}"#, "{"] {
        std::fs::write(path.join("manifest.json"), bytes).unwrap();
        assert!(scope_ids(path).is_err());
    }
    std::fs::remove_file(path.join("manifest.json")).unwrap();
    assert!(scope_ids(path).is_err());
}
