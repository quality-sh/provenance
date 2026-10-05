use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::ScopeId;
use provenance_macros::verifies;
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
use serde_json::{json, Value};

fn input<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

fn files(directory: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = Utf8PathBuf::from_path_buf(entry.unwrap().path()).unwrap();
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }
    found
}

/// Creates a record of each kind and then changes it, each through its public
/// writer.
fn create_and_update_every_kind(store: &StateStore, scope: &ScopeId) {
    store
        .apply_typed_spec(
            scope,
            input(json!({"schema_version":2, "spec":"files", "declared_by":"spec://files",
                "requirements":[{"key":"base", "id":"req_files", "statement":"The system stores files."}]})),
        )
        .unwrap();
    store
        .update_requirement(input(json!({"scope_id":"default", "id":"req_files",
            "declared_by":"spec://files", "status":"active"})))
        .unwrap();
    store
        .create_source(input(
            json!({"scope_id":"default", "id":"source_files", "name":"Policy A",
            "source_type":"policy", "url":null, "reference":null, "commit_pin":null,
            "effective_date":null, "review_date":null, "supersedes":[],
            "origin_thread":null, "origin_message":null}),
        ))
        .unwrap();
    store
        .update_source(input(
            json!({"scope_id":"default", "id":"source_files", "name":"Policy B"}),
        ))
        .unwrap();
    store
        .create_domain(input(
            json!({"scope_id":"default", "id":"domain_files", "name":"Domain A",
            "description":null, "color":null}),
        ))
        .unwrap();
    store
        .update_domain(input(
            json!({"scope_id":"default", "id":"domain_files", "name":"Domain B"}),
        ))
        .unwrap();
    store
        .create_resolution(input(json!({"scope_id":"default", "id":"resolution_files",
            "title":"Keep files", "requirement_ids":["req_files"], "supersedes":[],
            "position":"Keep files.", "rationale":"Files matter.", "status":"proposed",
            "context":null, "enforcement":null, "confidence":null, "inputs":[], "made_by":null,
            "approved_by":null, "approved_at":null, "origin_thread":null, "origin_message":null})))
        .unwrap();
    store
        .update_resolution(input(json!({"scope_id":"default", "id":"resolution_files",
            "position":"Keep every file."})))
        .unwrap();
    store
        .create_rule(input(
            json!({"scope_id":"default", "id":"rule_files", "name":null,
            "description":null, "requirement_ids":["req_files"], "resolution_ids":[],
            "statement":"The system keeps files.", "status":"active", "severity":"medium",
            "source_document":null, "source_section":null, "origin_thread":null,
            "origin_message":null}),
        ))
        .unwrap();
    store
        .update_rule(input(json!({"scope_id":"default", "id":"rule_files",
            "statement":"The system keeps every file."})))
        .unwrap();
    store
        .create_boundary(input(json!({"scope_id":"default", "id":"boundary_files",
            "requirement_id":"req_files", "statement":"Boundary A", "source_ref":null})))
        .unwrap();
    store
        .update_boundary(input(json!({"scope_id":"default", "id":"boundary_files",
            "statement":"Boundary B"})))
        .unwrap();
    store
        .create_topic(input(json!({"scope_id":"default", "id":"topic_files",
            "requirement_id":"req_files", "title":"Topic A", "status":"open", "links":[]})))
        .unwrap();
    store
        .edit_topic(input(
            json!({"scope_id":"default", "id":"topic_files", "title":"Topic B"}),
        ))
        .unwrap();
    store
        .create_question(input(json!({"scope_id":"default", "id":"question_files",
            "topic_id":"topic_files", "question":"Is A correct?", "resolution_method":"research",
            "status":"open", "answer":null, "links":[], "resolution_id":null, "contradicts":null})))
        .unwrap();
    store
        .edit_question(input(json!({"scope_id":"default", "id":"question_files",
            "question":"Is B correct?"})))
        .unwrap();
}

#[test]
#[verifies("rule_review_writes_change_only_record_files", examples)]
fn record_saves_change_only_record_files() {
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

    let manifest = std::fs::read(layout.manifest_path()).unwrap();
    create_and_update_every_kind(&store, &scope);
    assert_eq!(std::fs::read(layout.manifest_path()).unwrap(), manifest);

    let scope_dir = layout.scopes_dir().join("default");
    let added = files(&layout.state_dir())
        .into_iter()
        .filter(|path| path != &layout.manifest_path())
        .filter(|path| !path.starts_with(&scope_dir) || path.extension() != Some("jsonl"))
        .collect::<Vec<_>>();
    assert_eq!(added, [] as [Utf8PathBuf; 0]);
    assert!(!scope_dir.join("review").exists());
}
