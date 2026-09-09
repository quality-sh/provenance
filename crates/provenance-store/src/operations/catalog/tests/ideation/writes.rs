//! The creations keep native refusals, scope equality, lock serialization,
//! and the uncertainty of a publication that fails after it starts.
use super::super::super::{invoke_typed, CreateAssertion, CreateDisposition, CreateProposal};
use super::*;
use provenance_core::{CanonicalArtifact, CanonicalArtifactType};

#[tokio::test]
async fn request_scope_must_equal_the_selected_scope_and_leaves_no_effects() {
    let (dir, store, scope) = initialized();
    let mut input = proposal_input(&scope, "proposal_escape");
    input.scope_id = ScopeId::new("other").unwrap();
    let error = invoke_typed::<CreateProposal>(prepared(&dir, &scope), input)
        .await
        .unwrap_err();
    assert_eq!(refusal_kind(error), "scope_mismatch");
    let mut disposition = rejected_disposition(&scope, "disposition_escape", "proposal_absent");
    disposition.scope_id = ScopeId::new("other").unwrap();
    let error = invoke_typed::<CreateDisposition>(prepared(&dir, &scope), disposition)
        .await
        .unwrap_err();
    assert_eq!(refusal_kind(error), "scope_mismatch");
    assert!(store.list_proposal_cards(&scope).unwrap().is_empty());
    assert!(!store.layout.scopes_dir().join("other").exists());
}

#[tokio::test]
async fn disposition_actor_allowlist_and_its_empty_list_are_retained() {
    let (dir, store, scope) = initialized();
    store
        .create_proposal_card(proposal_input(&scope, "proposal_overtime"))
        .unwrap();
    let empty = invoke_typed::<CreateDisposition>(
        prepared(&dir, &scope),
        rejected_disposition(&scope, "disposition_one", "proposal_overtime"),
    )
    .await
    .unwrap_err();
    assert!(
        empty
            .to_string()
            .contains("no disposition actors configured"),
        "{empty}"
    );
    allow_actor(&store, "reviewer");
    let error = invoke_typed::<CreateDisposition>(
        prepared(&dir, &scope),
        CreateDispositionInput {
            actor: actor("forged-reviewer"),
            ..rejected_disposition(&scope, "disposition_one", "proposal_overtime")
        },
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("disposition actor is not in the repository allowlist"),
        "{error}"
    );
    assert!(store.list_dispositions(&scope).unwrap().is_empty());
}

#[tokio::test]
async fn duplicate_proposal_assertion_and_disposition_refusals_change_nothing() {
    let (dir, store, scope) = initialized();
    allow_actor(&store, "reviewer");
    invoke_typed::<CreateProposal>(
        prepared(&dir, &scope),
        proposal_input(&scope, "proposal_overtime"),
    )
    .await
    .unwrap();
    let duplicate = invoke_typed::<CreateProposal>(
        prepared(&dir, &scope),
        proposal_input(&scope, "proposal_overtime"),
    )
    .await
    .unwrap_err();
    assert!(duplicate.to_string().contains("immutable"), "{duplicate}");
    assert_eq!(store.list_proposal_definitions(&scope).unwrap().len(), 1);
    // Recreate the scope the assertion path needs: evidence, a proposal that
    // cites its claim, and a settled packet.
    let (dir, store, scope) = initialized();
    allow_actor(&store, "reviewer");
    seed_blocked_evidence(&store, &scope);
    let mut proposal = proposal_input(&scope, "proposal_overtime");
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    invoke_typed::<CreateProposal>(prepared(&dir, &scope), proposal)
        .await
        .unwrap();
    qualify_seeded_evidence(&store, &scope);
    invoke_typed::<CreateAssertion>(prepared(&dir, &scope), supported_assertion(&scope))
        .await
        .unwrap();
    let duplicate_assertion =
        invoke_typed::<CreateAssertion>(prepared(&dir, &scope), supported_assertion(&scope))
            .await
            .unwrap_err();
    assert!(
        duplicate_assertion.to_string().contains("already exists"),
        "{duplicate_assertion}"
    );
    invoke_typed::<CreateDisposition>(
        prepared(&dir, &scope),
        rejected_disposition(&scope, "disposition_one", "proposal_overtime"),
    )
    .await
    .unwrap();
    let duplicate_disposition = invoke_typed::<CreateDisposition>(
        prepared(&dir, &scope),
        rejected_disposition(&scope, "disposition_two", "proposal_overtime"),
    )
    .await
    .unwrap_err();
    assert!(
        duplicate_disposition
            .to_string()
            .contains("authoritative disposition"),
        "{duplicate_disposition}"
    );
    assert_eq!(store.list_dispositions(&scope).unwrap().len(), 1);
    assert_eq!(store.list_assertion_records(&scope).unwrap().len(), 1);
}

#[tokio::test]
async fn acceptance_needs_prior_assertion_except_the_human_canonical_artifact_exception() {
    let (dir, store, scope) = initialized();
    seed_requirement(&store, &scope);
    allow_actor(&store, "reviewer");
    store
        .create_proposal_card(proposal_input(&scope, "proposal_overtime"))
        .unwrap();
    let accepted = CreateDispositionInput {
        decision: DispositionDecision::Accepted,
        ..rejected_disposition(&scope, "disposition_one", "proposal_overtime")
    };
    let error = invoke_typed::<CreateDisposition>(prepared(&dir, &scope), accepted)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("accepted proposal must be asserted before disposition"),
        "{error}"
    );
    // The existing exception: a person who ratified an artifact stands on it.
    let ratified = CreateDispositionInput {
        decision: DispositionDecision::Accepted,
        canonical_artifact: Some(CanonicalArtifact {
            artifact_type: CanonicalArtifactType::Requirement,
            artifact_id: StableId::new("req_overtime").unwrap(),
        }),
        ..rejected_disposition(&scope, "disposition_one", "proposal_overtime")
    };
    let landed = invoke_typed::<CreateDisposition>(prepared(&dir, &scope), ratified)
        .await
        .unwrap();
    assert_eq!(landed.decision, DispositionDecision::Accepted);
    assert_eq!(store.list_dispositions(&scope).unwrap().len(), 1);
}

#[tokio::test]
async fn missing_proposal_and_missing_canonical_artifact_are_refused_before_publication() {
    let (dir, store, scope) = initialized();
    seed_requirement(&store, &scope);
    allow_actor(&store, "reviewer");
    let ghost = invoke_typed::<CreateDisposition>(
        prepared(&dir, &scope),
        rejected_disposition(&scope, "disposition_ghost", "proposal_ghost"),
    )
    .await
    .unwrap_err();
    assert!(
        ghost.to_string().contains("proposal does not exist"),
        "{ghost}"
    );
    store
        .create_proposal_card(proposal_input(&scope, "proposal_overtime"))
        .unwrap();
    let missing_artifact = CreateDispositionInput {
        decision: DispositionDecision::Accepted,
        canonical_artifact: Some(CanonicalArtifact {
            artifact_type: CanonicalArtifactType::Requirement,
            artifact_id: StableId::new("req_missing").unwrap(),
        }),
        ..rejected_disposition(&scope, "disposition_missing", "proposal_overtime")
    };
    let error = invoke_typed::<CreateDisposition>(prepared(&dir, &scope), missing_artifact)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("canonical artifact"), "{error}");
    assert!(store.list_dispositions(&scope).unwrap().is_empty());
}

#[tokio::test]
async fn empty_rationale_is_refused_with_the_native_words() {
    let (dir, store, scope) = initialized();
    allow_actor(&store, "reviewer");
    store
        .create_proposal_card(proposal_input(&scope, "proposal_overtime"))
        .unwrap();
    let input = CreateDispositionInput {
        rationale: "   ".into(),
        ..rejected_disposition(&scope, "disposition_one", "proposal_overtime")
    };
    let error = invoke_typed::<CreateDisposition>(prepared(&dir, &scope), input)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("disposition rationale must not be empty"),
        "{error}"
    );
    assert!(store.list_dispositions(&scope).unwrap().is_empty());
}

#[tokio::test]
async fn an_assertion_cannot_reenter_a_disposed_proposal() {
    let (dir, store, scope) = initialized();
    allow_actor(&store, "reviewer");
    seed_blocked_evidence(&store, &scope);
    let mut proposal = proposal_input(&scope, "proposal_overtime");
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    invoke_typed::<CreateProposal>(prepared(&dir, &scope), proposal)
        .await
        .unwrap();
    invoke_typed::<CreateDisposition>(
        prepared(&dir, &scope),
        rejected_disposition(&scope, "disposition_one", "proposal_overtime"),
    )
    .await
    .unwrap();
    let error =
        invoke_typed::<CreateAssertion>(prepared(&dir, &scope), supported_assertion(&scope))
            .await
            .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("disposed proposal cannot re-enter assertion"),
        "{error}"
    );
    assert!(store.list_assertion_records(&scope).unwrap().is_empty());
}

#[tokio::test]
async fn concurrent_dispositions_through_the_catalog_exactly_one_wins() {
    let (dir, store, scope) = initialized();
    allow_actor(&store, "reviewer");
    invoke_typed::<CreateProposal>(
        prepared(&dir, &scope),
        proposal_input(&scope, "proposal_overtime"),
    )
    .await
    .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads = ["disposition_a", "disposition_b"].map(|id| {
        let dir = dir.path().to_path_buf();
        let scope = scope.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let root = camino::Utf8PathBuf::from_path_buf(dir).unwrap();
                    invoke_typed::<CreateDisposition>(
                        PreparedContext::for_scope(PreparedScope {
                            root,
                            scope: scope.clone(),
                            requested_target: "selected".into(),
                        }),
                        rejected_disposition(&scope, id, "proposal_overtime"),
                    )
                    .await
                })
        })
    });
    let results = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "{results:?}"
    );
    assert_eq!(store.list_dispositions(&scope).unwrap().len(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn a_publication_that_cannot_write_reports_uncertainty() {
    use std::os::unix::fs::PermissionsExt;

    let (dir, store, scope) = initialized();
    allow_actor(&store, "reviewer");
    invoke_typed::<CreateProposal>(
        prepared(&dir, &scope),
        proposal_input(&scope, "proposal_overtime"),
    )
    .await
    .unwrap();
    let dispositions_dir = crate::shards::dispositions_path(&store.layout, &scope)
        .parent()
        .unwrap()
        .to_owned();
    std::fs::create_dir_all(&dispositions_dir).unwrap();
    let mut permissions = std::fs::metadata(&dispositions_dir).unwrap().permissions();
    permissions.set_mode(0o555);
    std::fs::set_permissions(&dispositions_dir, permissions.clone()).unwrap();
    let result = invoke_typed::<CreateDisposition>(
        prepared(&dir, &scope),
        rejected_disposition(&scope, "disposition_one", "proposal_overtime"),
    )
    .await;
    permissions.set_mode(0o755);
    std::fs::set_permissions(&dispositions_dir, permissions).unwrap();
    assert_eq!(refusal_kind(result.unwrap_err()), "uncertain_write");
    assert!(store.list_dispositions(&scope).unwrap().is_empty());
}
