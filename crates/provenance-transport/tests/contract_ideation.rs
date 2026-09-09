//! The proposal-lifecycle operations keep native results, refusals, and
//! persistence across the HTTP and MCP adapters.
#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
#[allow(dead_code)]
mod records;
use provenance_core::ScopeId;
use records::{call, Repository};
use serde_json::{json, Value};

fn writable_host(repo: &Repository) -> provenance_transport::StatementHost {
    use provenance_transport::fixture::{FixtureAccess, Target};
    provenance_transport::StatementHost::with_fixture_access(
        FixtureAccess::new(
            vec![Target {
                id: "selected".into(),
                root: repo.dir.path().into(),
            }],
            vec![("selected".into(), "default".into())],
            "fixture-secret",
            "fixture.test",
        )
        .unwrap()
        .allow_writes(),
    )
}

fn scoped(request: &Value) -> Value {
    json!({"context":{"repository":"selected","scope":"default"},"request":request})
}

fn allow_actor(repo: &Repository, id: &str) {
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(repo.layout.manifest_path()).unwrap()).unwrap();
    manifest["disposition_actor_ids"]
        .as_array_mut()
        .unwrap()
        .push(json!(id));
    std::fs::write(
        repo.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

fn proposal() -> Value {
    json!({"scope_id":"default","id":"proposal_overtime","proposal_key":"overtime",
        "proposal_type":"requirement_candidate","title":"Overtime","summary":"Clarify overtime.",
        "traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_overtime"},
            "source_ids":[],"evidence_references":[],"supporting_claim_ids":["claim_overtime"]},
        "builds_on":[],"promotion_state":"proposed"})
}

fn assertion() -> Value {
    json!({"scope_id":"default","id":"assertion_overtime","proposal_id":"proposal_overtime",
        "synthesis_packet_id":"synthesis_overtime","supporting_claim_ids":["claim_overtime"]})
}

fn disposition(id: &str, decision: &str, actor: &str) -> Value {
    json!({"scope_id":"default","id":id,"proposal_id":"proposal_overtime","decision":decision,
        "rationale":"Reviewed","actor":{"identity_type":"human","id":actor}})
}

/// Seed one contribution and one synthesis packet whose blocking evidence gap
/// keeps the bare proposal row from demanding an assertion; clearing the gap
/// afterwards makes the packet qualify the proposal.
fn seed_evidence(repo: &Repository, blocking: bool) {
    let scope = ScopeId::new("default").unwrap();
    let contribution = json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION, "scope_id": "default",
        "id": "contribution_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_overtime", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_overtime", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_overtime"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"},
        "open_questions": []
    });
    let mut packet = json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION, "scope_id": "default",
        "id": "synthesis_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [], "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_overtime", "proposal_key": "overtime",
            "proposal_type": "requirement_candidate", "summary": "Candidate",
            "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []
    });
    if blocking {
        packet["evidence_gaps"] = json!([{"question": "Unverified", "needed_evidence_type": "source", "blocking_promotion": true}]);
    }
    for (path, value) in [
        (
            provenance_store::shards::contributions_path(&repo.layout, &scope),
            contribution,
        ),
        (
            provenance_store::shards::synthesis_packets_path(&repo.layout, &scope),
            packet,
        ),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("{value}\n")).unwrap();
    }
}

fn shard(repo: &Repository, name: &str) -> Value {
    let scope = ScopeId::new("default").unwrap();
    let path = match name {
        "proposal_cards" => provenance_store::shards::proposal_cards_path(&repo.layout, &scope),
        "assertions" => provenance_store::shards::assertion_records_path(&repo.layout, &scope),
        "dispositions" => provenance_store::shards::dispositions_path(&repo.layout, &scope),
        _ => panic!("unknown shard {name}"),
    };
    if !path.exists() {
        return json!([]);
    }
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>()
        .into()
}

fn snapshot(repo: &Repository) -> Value {
    json!([
        shard(repo, "proposal_cards"),
        shard(repo, "assertions"),
        shard(repo, "dispositions")
    ])
}

#[tokio::test]
async fn ideation_operations_preserve_native_results_and_persistence() {
    use provenance_store::state_store::StateStore;
    let native = Repository::new("The shared graph is readable.");
    let repo = Repository::new("The shared graph is readable.");
    for target in [&native, &repo] {
        allow_actor(target, "reviewer");
        seed_evidence(target, true);
    }
    let host = writable_host(&repo);
    let store = StateStore::new(native.layout.clone());
    let scope = ScopeId::new("default").unwrap();
    let (status, created) = call(&host, "create-proposal", scoped(&proposal())).await;
    assert_eq!(status, 200, "{created}");
    assert_eq!(
        created,
        json!(store
            .create_proposal_card(serde_json::from_value(proposal()).unwrap())
            .unwrap())
    );
    seed_evidence(&repo, false);
    seed_evidence(&native, false);
    let (status, asserted) = call(&host, "create-assertion", scoped(&assertion())).await;
    assert_eq!(status, 200, "{asserted}");
    assert_eq!(
        asserted,
        json!(store
            .assert_proposal(serde_json::from_value(assertion()).unwrap())
            .unwrap())
    );
    let (status, disposed) = call(
        &host,
        "create-disposition",
        scoped(&disposition("disposition_overtime", "rejected", "reviewer")),
    )
    .await;
    assert_eq!(status, 200, "{disposed}");
    assert_eq!(
        disposed,
        json!(store
            .create_disposition(
                serde_json::from_value(disposition("disposition_overtime", "rejected", "reviewer"))
                    .unwrap()
            )
            .unwrap())
    );
    for (operation, expected) in [
        (
            "list-proposals",
            json!(store.list_proposal_cards(&scope).unwrap()),
        ),
        (
            "list-dispositions",
            json!(store.list_dispositions(&scope).unwrap()),
        ),
        (
            "list-assertions",
            json!(store.list_assertion_records(&scope).unwrap()),
        ),
    ] {
        let (status, listed) = call(&host, operation, scoped(&Value::Null)).await;
        assert_eq!(status, 200, "{listed}");
        assert_eq!(listed, expected, "{operation}");
    }
    let listed = call(&host, "list-proposals", scoped(&Value::Null)).await.1;
    assert_eq!(listed[0]["promotion_state"], "rejected");
    host.shutdown().await;
    let actual = StateStore::new(repo.layout.clone());
    assert_eq!(
        json!(actual.list_proposal_definitions(&scope).unwrap()),
        json!(store.list_proposal_definitions(&scope).unwrap())
    );
    assert_eq!(
        actual.list_proposal_definitions(&scope).unwrap()[0].promotion_state,
        provenance_core::PromotionState::Proposed
    );
    assert_eq!(
        actual.list_dispositions(&scope).unwrap(),
        store.list_dispositions(&scope).unwrap()
    );
    assert_eq!(
        actual.list_assertion_records(&scope).unwrap(),
        store.list_assertion_records(&scope).unwrap()
    );
}

#[tokio::test]
async fn ideation_refusals_keep_the_wire_families_and_have_no_effects() {
    let repo = Repository::new("The shared graph is readable.");
    allow_actor(&repo, "reviewer");
    seed_evidence(&repo, true);
    let host = writable_host(&repo);
    let (status, created) = call(&host, "create-proposal", scoped(&proposal())).await;
    assert_eq!(status, 200, "{created}");
    let (status, disposed) = call(
        &host,
        "create-disposition",
        scoped(&disposition("disposition_first", "rejected", "reviewer")),
    )
    .await;
    assert_eq!(status, 200, "{disposed}");
    let before = snapshot(&repo);
    let mut mismatch = proposal();
    mismatch["scope_id"] = json!("other");
    let mut empty_rationale = disposition("disposition_next", "rejected", "reviewer");
    empty_rationale["rationale"] = json!("   ");
    let mut accepted = disposition("disposition_accept", "accepted", "reviewer");
    accepted["canonical_artifact"] =
        json!({"artifact_type":"requirement","artifact_id":"req_absent"});
    let forged = disposition("disposition_forged", "rejected", "forged-reviewer");
    let cases = [
        ("create-proposal", mismatch, 400, "scope_mismatch"),
        ("create-disposition", empty_rationale, 500, "write_failed"),
        ("create-disposition", accepted, 500, "write_failed"),
        ("create-disposition", forged, 500, "write_failed"),
        ("create-proposal", proposal(), 500, "write_failed"),
        (
            "create-disposition",
            disposition("disposition_second", "rejected", "reviewer"),
            500,
            "write_failed",
        ),
        ("create-assertion", assertion(), 500, "write_failed"),
    ];
    for (operation, request, expected_status, expected_kind) in cases {
        let (status, failure) = call(&host, operation, scoped(&request)).await;
        assert_eq!(status, expected_status, "{operation}: {failure}");
        assert_eq!(failure["error"]["kind"], expected_kind, "{operation}");
        assert_eq!(snapshot(&repo), before, "{operation}");
        assert!(!repo.layout.scopes_dir().join("other").exists());
    }
    host.shutdown().await;
}

#[tokio::test]
async fn assertion_refusal_before_publication_leaves_the_shard_exactly_as_found() {
    let repo = Repository::new("The shared graph is readable.");
    allow_actor(&repo, "reviewer");
    seed_evidence(&repo, true);
    let host = writable_host(&repo);
    let (status, created) = call(&host, "create-proposal", scoped(&proposal())).await;
    assert_eq!(status, 200, "{created}");
    seed_evidence(&repo, false);
    let path = provenance_store::shards::assertion_records_path(
        &repo.layout,
        &ScopeId::new("default").unwrap(),
    );
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "invalid JSON\n").unwrap();
    let (status, failure) = call(&host, "create-assertion", scoped(&assertion())).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(failure["error"]["kind"], "write_failed");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid JSON\n");
    host.shutdown().await;
}

#[tokio::test]
async fn concurrent_dispositions_over_http_exactly_one_wins() {
    let repo = Repository::new("The shared graph is readable.");
    allow_actor(&repo, "reviewer");
    let host = writable_host(&repo);
    let (status, created) = call(&host, "create-proposal", scoped(&proposal())).await;
    assert_eq!(status, 200, "{created}");
    let first = tokio::spawn({
        let host = host.clone();
        async move {
            call(
                &host,
                "create-disposition",
                scoped(&disposition("disposition_a", "rejected", "reviewer")),
            )
            .await
        }
    });
    let second = call(
        &host,
        "create-disposition",
        scoped(&disposition("disposition_b", "rejected", "reviewer")),
    );
    let (first, second) = tokio::join!(first, second);
    let results = [first.unwrap(), second];
    assert_eq!(
        results.iter().filter(|(status, _)| *status == 200).count(),
        1,
        "{results:?}"
    );
    assert_eq!(shard(&repo, "dispositions").as_array().unwrap().len(), 1);
    host.shutdown().await;
}

#[tokio::test]
async fn mcp_ideation_advertises_reads_and_refuses_writes_without_a_write_grant() {
    use rmcp::{model::CallToolRequestParams, ServiceExt};
    let repo = Repository::new("The shared graph is readable.");
    allow_actor(&repo, "reviewer");
    seed_evidence(&repo, true);
    let writes = [
        ("create-proposal", scoped(&proposal())),
        ("create-assertion", scoped(&assertion())),
        (
            "create-disposition",
            scoped(&disposition("disposition_mcp", "rejected", "reviewer")),
        ),
    ];
    for writable in [false, true] {
        let host = if writable {
            writable_host(&repo)
        } else {
            records::host(&[("selected", &repo)], &["selected"])
        };
        let (client_io, server_io) = tokio::io::duplex(256 * 1024);
        let server_host = host.clone();
        let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
        let client = ().serve(client_io).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        for name in ["list-proposals", "list-dispositions", "list-assertions"] {
            assert!(tools.iter().any(|tool| tool.name == *name), "{name}");
        }
        for (name, _) in &writes {
            assert_eq!(
                tools.iter().any(|tool| tool.name == *name),
                writable,
                "{name}"
            );
        }
        for (name, body) in &writes {
            let result = client
                .call_tool(
                    CallToolRequestParams::new((*name).to_owned()).with_arguments(
                        json!({"protocol_version":7,"call":body})
                            .as_object()
                            .unwrap()
                            .clone(),
                    ),
                )
                .await
                .unwrap();
            if *name == "create-proposal" && writable {
                // Settle the run's adjudication only after the proposal row
                // exists, so the later writes are admissible.
                seed_evidence(&repo, false);
            }
            if writable {
                assert_ne!(result.is_error, Some(true), "{name}: {result:?}");
            } else {
                assert_eq!(result.is_error, Some(true), "{name}");
                assert_eq!(
                    result.structured_content.unwrap()["error"]["kind"],
                    "access_denied"
                );
            }
        }
        if !writable {
            assert_eq!(snapshot(&repo), json!([json!([]), json!([]), json!([])]));
        }
        let store = provenance_store::state_store::StateStore::new(repo.layout.clone());
        let scope = ScopeId::new("default").unwrap();
        for (name, expected) in [
            (
                "list-proposals",
                json!(store.list_proposal_cards(&scope).unwrap()),
            ),
            (
                "list-dispositions",
                json!(store.list_dispositions(&scope).unwrap()),
            ),
            (
                "list-assertions",
                json!(store.list_assertion_records(&scope).unwrap()),
            ),
        ] {
            let result = client
                .call_tool(
                    CallToolRequestParams::new((*name).to_owned()).with_arguments(
                        json!({"protocol_version":7,"call":scoped(&Value::Null)})
                            .as_object()
                            .unwrap()
                            .clone(),
                    ),
                )
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true), "{name}: {result:?}");
            assert_eq!(
                result.structured_content.unwrap(),
                json!({"result": expected})
            );
        }
        client.cancel().await.unwrap();
        server.await.unwrap().cancel().await.unwrap();
        host.shutdown().await;
    }
}
