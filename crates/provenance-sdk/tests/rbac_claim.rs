//! The SDK's claim transport: the environment's `PROVENANCE_ACTOR_ID` rides
//! every verification mutation the SDK facade makes, so an rbac-managed
//! repository can authorize the runs it records.

use provenance_core::{RuleSeverity, RuleStatus, ScopeId, StableId, MISSING_CLAIM_REFUSAL};
use provenance_sdk::verify;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::{CreateRuleInput, StateStore};
use std::sync::Mutex;

/// Environment mutation is process-global; the SDK reads it per call.
static ENV: Mutex<()> = Mutex::new(());

fn rbac_repository(grant_reviewer: bool) -> (tempfile::TempDir, camino::Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().canonicalize().unwrap()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir().as_std_path()).unwrap();
    let grants = if grant_reviewer {
        r#",
    "rbac": {"assignments": [{
        "actor_id": "reviewer",
        "identity_type": "human",
        "capabilities": ["edit", "execute"],
        "scopes": ["default"]
    }]}"#
    } else {
        r#",
    "rbac": {"assignments": [{
        "actor_id": "someone-else",
        "identity_type": "human",
        "capabilities": ["edit", "execute"],
        "scopes": ["default"]
    }]}"#
    };
    std::fs::write(
        layout.manifest_path(),
        format!(
            r#"{{"schema_version": 1,
    "scopes": [{{"id": "default", "path_prefix": "."}}],
    "disposition_actor_ids": []{grants}}}"#
        ),
    )
    .unwrap();
    // The verification targets an existing rule; the seeder holds `edit`.
    let store = StateStore::new(layout);
    let seeder = if grant_reviewer {
        provenance_core::RbacClaim::new("reviewer").unwrap()
    } else {
        provenance_core::RbacClaim::new("someone-else").unwrap()
    };
    store
        .create_rule(
            Some(&seeder),
            CreateRuleInput {
                scope_id: ScopeId::new("default").unwrap(),
                id: StableId::new("rule_expiry").unwrap(),
                name: None,
                description: None,
                requirement_id: None,
                resolution_id: None,
                statement: "Share links expire within 30 days".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    (dir, root)
}

#[test]
fn the_environment_actor_rides_the_sdk_verification_mutations() {
    let _guard = ENV.lock().unwrap();
    let (dir, root) = rbac_repository(true);
    // SAFETY of the test: the ENV mutex serializes every mutation.
    std::env::set_var("PROVENANCE_REPO", root.as_std_path());
    std::env::remove_var("PROVENANCE_ACTOR_ID");

    // Without a claim the rbac repository refuses the run.
    let error = verify(
        "rule_expiry",
        "sdk-claim-transport",
        || Ok::<(), String>(()),
    )
    .expect_err("a claimless run must refuse");
    assert_eq!(error.to_string(), MISSING_CLAIM_REFUSAL);

    // The configured actor rides the begin and complete mutations.
    std::env::set_var("PROVENANCE_ACTOR_ID", "reviewer");
    verify(
        "rule_expiry",
        "sdk-claim-transport",
        || Ok::<(), String>(()),
    )
    .expect("the granted actor must record the run");

    // An unauthorized actor refuses with the wrong-principal golden.
    std::env::set_var("PROVENANCE_ACTOR_ID", "intruder");
    let error = verify(
        "rule_expiry",
        "sdk-claim-transport",
        || Ok::<(), String>(()),
    )
    .expect_err("an unauthorized run must refuse");
    assert_eq!(
        error.to_string(),
        "rbac: actor intruder does not hold capability execute on scope default"
    );

    std::env::remove_var("PROVENANCE_REPO");
    std::env::remove_var("PROVENANCE_ACTOR_ID");
    drop(dir);
}
