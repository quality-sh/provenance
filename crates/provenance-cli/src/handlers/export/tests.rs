use super::export_scope;
use camino::Utf8PathBuf;
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_store::layout::ProvenanceLayout;
use serde_json::json;

fn fixture() -> (tempfile::TempDir, Utf8PathBuf, ProvenanceLayout) {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    let scope = ScopeId::new("default").unwrap();
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope,
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    (temp, root, layout)
}

#[test]
fn export_refuses_an_unknown_record_field_instead_of_dropping_it() {
    let (_temp, root, layout) = fixture();
    let path = layout
        .state_dir()
        .join("scopes/default/sources/source.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            "{}\n",
            json!({
                "schema_version": 2,
                "scope_id": "default",
                "id": "source_one",
                "name": "Policy",
                "source_type": "policy",
                "url": null,
                "extension": {"owner": "newer-tool"}
            })
        ),
    )
    .unwrap();

    let message = match export_scope(root, "default".into()) {
        Ok(_) => panic!("export accepted a record field that it cannot represent"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("unknown field `extension`"), "{message}");
}

#[test]
fn export_refuses_a_duplicate_record_field() {
    let (_temp, root, layout) = fixture();
    let path = layout
        .state_dir()
        .join("scopes/default/sources/source.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        concat!(
            r#"{"schema_version":2,"scope_id":"default","id":"source_one","name":"First","name":"Second","source_type":"policy","url":null}"#,
            "\n"
        ),
    )
    .unwrap();

    let message = match export_scope(root, "default".into()) {
        Ok(_) => panic!("export accepted an ambiguous record"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("duplicate field `name`"), "{message}");
}

#[test]
fn export_refuses_an_unknown_landing_field_instead_of_dropping_it() {
    let (_temp, root, layout) = fixture();
    let path = layout
        .state_dir()
        .join("scopes/default/ideation/landings.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            "{}\n",
            json!({
                "contributions": [],
                "synthesis_packets": [],
                "proposals": [],
                "assertions": [],
                "dispositions": [],
                "extension": {"owner": "newer-tool"}
            })
        ),
    )
    .unwrap();

    let message = match export_scope(root, "default".into()) {
        Ok(_) => panic!("export accepted a landing field that it cannot represent"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("unknown field `extension`"), "{message}");
}
