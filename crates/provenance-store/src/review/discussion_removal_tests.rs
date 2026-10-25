use super::WriteDiscussion;
use crate::{
    layout::ProvenanceLayout,
    shards,
    state_store::{StateStore, TypedSpecInput},
};
use camino::Utf8Path;
use provenance_core::{protocol::TypedSourceInput, ScopeId, SUPPORTED_SCHEMA_VERSION};
use serde_json::json;

const OWNER: &str = "spec://review/removal";

fn spec(sources: Vec<TypedSourceInput>) -> TypedSpecInput {
    TypedSpecInput {
        schema_version: SUPPORTED_SCHEMA_VERSION.0,
        spec: "removal".into(),
        declared_by: OWNER.into(),
        adopt_unowned: Vec::new(),
        sources,
        requirements: Vec::new(),
        rules: Vec::new(),
    }
}

#[test]
fn a_removed_message_outside_the_record_file_refuses_before_any_write() {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let store = StateStore::new(layout.clone());
    let scope = ScopeId::new("default").unwrap();
    let source = TypedSourceInput {
        key: "policy".into(),
        id: None,
        name: "Policy".into(),
        kind: "document".into(),
        url: None,
        reference: None,
        supersedes: None,
    };
    store.apply_typed_spec(&scope, spec(vec![source])).unwrap();
    let source_id = store.list_sources(&scope).unwrap()[0].id.clone();
    let start: WriteDiscussion = serde_json::from_value(json!({
        "scope_id":"default", "parent":{"node_type":"source", "node_id":source_id},
        "actor":"reviewer", "declared_by":OWNER,
        "action":{"kind":"start", "role":"user", "body":"A concern."}
    }))
    .unwrap();
    store.write_discussion(start).unwrap();
    // Move the Message to a month shard that the publication guard does not allow.
    let july = shards::messages_path(&layout, &scope);
    let september = july.with_file_name("2026-09.jsonl");
    std::fs::rename(&july, &september).unwrap();
    std::fs::write(&july, "").unwrap();
    let before = files(&layout);

    let error = store
        .apply_typed_spec(&scope, spec(Vec::new()))
        .unwrap_err();

    assert!(
        format!("{error:#}").contains(
            "a Message of one is in scopes/default/threads/2026-09.jsonl, \
             which typed-spec apply cannot change"
        ),
        "{error:#}"
    );
    assert_eq!(files(&layout), before);
}

fn files(layout: &ProvenanceLayout) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut directories = vec![layout.state_dir().into_std_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else {
                files.push((path.display().to_string(), std::fs::read(path).unwrap()));
            }
        }
    }
    files.sort();
    files
}
