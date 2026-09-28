use super::{entry_path, new_id, snapshot, write_new};
use crate::{cache::review_families, layout::ProvenanceLayout, state_store::StateStore};
use camino::Utf8Path;
use provenance_core::{
    review::{ReviewEntry, SaveOutcome, REVIEW_SCHEMA_VERSION},
    NodeType, ScopeId, StableId,
};
use serde_json::{json, Value};

fn fixtures() -> [(NodeType, Value); 8] {
    [
        (NodeType::Source, json!({"schema_version":3,"scope_id":"default","id":"source_a","name":"Policy","source_type":"document","url":null})),
        (NodeType::Requirement, json!({"schema_version":3,"scope_id":"default","id":"req_a","statement":"The system stores records.","status":"active"})),
        (NodeType::Resolution, json!({"schema_version":3,"scope_id":"default","id":"resolution_a","title":"Decision","position":"Use it.","rationale":"It is required.","status":"approved","inputs":[],"requirement_ids":["req_a"],"review_on":null})),
        (NodeType::Rule, json!({"schema_version":3,"scope_id":"default","id":"rule_a","statement":"The system stores records.","status":"active","severity":"high","requirement_ids":["req_a"],"resolution_ids":[]})),
        (NodeType::Domain, json!({"schema_version":3,"scope_id":"default","id":"domain_a","name":"Storage"})),
        (NodeType::Boundary, json!({"schema_version":3,"scope_id":"default","id":"boundary_a","requirement_id":"req_a","statement":"Storage only."})),
        (NodeType::Topic, json!({"schema_version":3,"scope_id":"default","id":"topic_a","requirement_id":"req_a","title":"Storage","status":"open","links":[]})),
        (NodeType::Question, json!({"schema_version":3,"scope_id":"default","id":"question_a","topic_id":"topic_a","requirement_id":"req_a","question":"Where is storage?","resolution_method":"research","status":"open","links":[]})),
    ]
}

fn entry(
    kind: NodeType,
    record_id: StableId,
    sequence: u64,
    predecessor: Option<&ReviewEntry>,
    after: provenance_core::review::SnapshotRef,
) -> ReviewEntry {
    ReviewEntry {
        schema_version: REVIEW_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        record_kind: kind,
        record_id,
        id: new_id(),
        sequence,
        predecessor: predecessor.map(|value| value.id.clone()),
        revision: new_id(),
        prior_revision: predecessor.map(|value| value.revision.clone()),
        before: predecessor.map(|value| value.after.clone()),
        after,
        changed_fields: vec![
            review_families::by_kind(kind).content_fields[0].to_string(),
        ],
        actor: "reviewer".into(),
        request_id: new_id(),
        intent_digest: format!("sha256:intent-{sequence}"),
        etag: format!("sha256:etag-{sequence}"),
        outcome: SaveOutcome::Changed,
        origin: None,
    }
}

#[test]
fn every_record_kind_has_readable_snapshots_and_revision_chains() {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    let store = StateStore::new(layout.clone());
    let scope = ScopeId::new("default").unwrap();
    let mut records = Vec::new();

    for (kind, value) in fixtures() {
        let record = review_families::deserialize_record(kind, value).unwrap();
        let first = entry(
            kind,
            record.id().clone(),
            1,
            None,
            snapshot(&layout, &record).unwrap(),
        );
        write_new(
            &entry_path(&layout, &scope, &first.request_id),
            &first,
        )
        .unwrap();
        let second = entry(
            kind,
            record.id().clone(),
            2,
            Some(&first),
            snapshot(&layout, &record).unwrap(),
        );
        write_new(
            &entry_path(&layout, &scope, &second.request_id),
            &second,
        )
        .unwrap();
        records.push(record);
    }

    let entries = store.validated_review_entries(&scope).unwrap();
    assert_eq!(entries.len(), NodeType::ALL.len() * 2);
    for record in records {
        let head = store.head(&record).unwrap().unwrap();
        assert_eq!(head.record_kind, record.kind());
        assert_eq!(head.record_id, *record.id());
        assert_eq!(head.sequence, 2);
    }
}
