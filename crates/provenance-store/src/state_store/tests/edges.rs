use super::seeded_requirement_store;
use crate::state_store::{CreateEdgeInput, CreateRequirementInput};
use provenance_core::{Edge, EdgeType, NodeType, RequirementStatus, SchemaVersion, StableId};

#[test]
fn generic_edges_validate_endpoints_and_delete() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_leave").unwrap(),
            statement: "Leave".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();

    let edge = store
        .create_edge(CreateEdgeInput {
            scope_id: scope.clone(),
            edge_type: EdgeType::RefinesInto,
            from_type: NodeType::Requirement,
            from_id: StableId::new("req_overtime").unwrap(),
            to_type: NodeType::Requirement,
            to_id: StableId::new("req_leave").unwrap(),
        })
        .unwrap();

    assert_eq!(edge.edge_type, EdgeType::RefinesInto);
    assert_eq!(store.list_edges().unwrap()[0].id, edge.id);

    let err = store
        .create_edge(CreateEdgeInput {
            scope_id: scope.clone(),
            edge_type: EdgeType::RefinesInto,
            from_type: NodeType::Requirement,
            from_id: StableId::new("req_overtime").unwrap(),
            to_type: NodeType::Requirement,
            to_id: StableId::new("req_missing").unwrap(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("to endpoint does not exist"));

    let deleted = store.delete_edge(&scope, &edge.id).unwrap();
    assert_eq!(deleted.id, edge.id);
    assert!(store.list_edges().unwrap().is_empty());
}

#[test]
fn list_edges_reads_all_edge_shards() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_leave").unwrap(),
            statement: "Leave".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let first_edge = store
        .create_edge(CreateEdgeInput {
            scope_id: scope.clone(),
            edge_type: EdgeType::RefinesInto,
            from_type: NodeType::Requirement,
            from_id: StableId::new("req_overtime").unwrap(),
            to_type: NodeType::Requirement,
            to_id: StableId::new("req_leave").unwrap(),
        })
        .unwrap();
    let second_edge = Edge {
        schema_version: SchemaVersion(1),
        scope_id: scope,
        id: StableId::new("edge_second_shard").unwrap(),
        edge_type: EdgeType::DependsOn,
        from_type: NodeType::Requirement,
        from_id: StableId::new("req_leave").unwrap(),
        to_type: NodeType::Requirement,
        to_id: StableId::new("req_overtime").unwrap(),
        label: None,
    };
    let second_shard = store.layout.edges_dir().join("edges-01.jsonl");
    std::fs::create_dir_all(second_shard.parent().unwrap()).unwrap();
    std::fs::write(
        second_shard,
        format!("{}\n", serde_json::to_string(&second_edge).unwrap()),
    )
    .unwrap();

    let edges = store.list_edges().unwrap();
    assert_eq!(edges.len(), 2);
    assert!(edges.iter().any(|edge| edge.id == first_edge.id));
    assert!(edges.iter().any(|edge| edge.id == second_edge.id));
}

#[test]
fn scope_publication_waits_for_edge_validation_and_insertion() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_leave").unwrap(),
            statement: "Leave".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let (validated_tx, validated_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let writer = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store.create_edge_after_validation(
                CreateEdgeInput {
                    scope_id: scope,
                    edge_type: EdgeType::RefinesInto,
                    from_type: NodeType::Requirement,
                    from_id: StableId::new("req_overtime").unwrap(),
                    to_type: NodeType::Requirement,
                    to_id: StableId::new("req_leave").unwrap(),
                },
                || {
                    validated_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(())
                },
            )
        })
    };
    validated_rx.recv().unwrap();

    let (published_tx, published_rx) = std::sync::mpsc::channel();
    let publisher = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store
                .with_repository_publication(|| {
                    crate::jsonl::write_jsonl_atomic::<provenance_core::Requirement>(
                        &crate::shards::requirements_path(&store.layout, &scope),
                        &[],
                    )?;
                    crate::jsonl::write_jsonl_atomic::<Edge>(
                        &crate::shards::edges_path(&store.layout),
                        &[],
                    )
                })
                .unwrap();
            published_tx.send(()).unwrap();
        })
    };

    assert!(published_rx
        .recv_timeout(std::time::Duration::from_millis(100))
        .is_err());
    release_tx.send(()).unwrap();
    writer.join().unwrap().unwrap();
    publisher.join().unwrap();

    assert!(store.list_requirements(&scope).unwrap().is_empty());
    assert!(store.list_edges().unwrap().is_empty());
}

#[test]
fn edge_endpoint_existence_refuses_domain_and_boundary_by_type() {
    // The endpoint table rejects these kinds before any existence lookup,
    // so the lookup's own answer is a typed refusal, never a scan.
    let (_dir, store, scope) = seeded_requirement_store();
    for kind in [NodeType::Domain, NodeType::Boundary] {
        let error = store
            .ensure_edge_endpoint_exists(&scope, kind, &StableId::new("domain_x").unwrap(), "from")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("never name"),
            "kind {kind:?} must be refused by type, got: {error}"
        );
    }
}
