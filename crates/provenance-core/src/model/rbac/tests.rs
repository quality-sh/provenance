//! Round-trip, refusal, and policy goldens for the manifest `rbac` section.

use super::policy::{
    authorize, ensure_disposition_actor_is_human, ensure_rbac_section_well_formed,
    ensure_unambiguous_rbac, AMBIGUOUS_MANIFEST_REFUSAL, MISSING_CLAIM_REFUSAL,
    RATIFICATION_REFUSAL_TAIL,
};
use super::types::{Assignment, Capability, RbacClaim, RbacResource, RbacSection};
use crate::{Manifest, RepoPathPrefix, ScopeId};
use serde_json::json;

fn scope_id(name: &str) -> ScopeId {
    ScopeId::new(name).unwrap()
}

fn section(assignments: Vec<Assignment>) -> RbacSection {
    RbacSection { assignments }
}

fn assignment(actor_id: &str, capabilities: &[Capability], scopes: &[&str]) -> Assignment {
    Assignment {
        actor_id: actor_id.to_string(),
        identity_type: None,
        capabilities: capabilities.to_vec(),
        scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
    }
}

#[test]
fn manifest_round_trips_an_rbac_section() {
    let body = r#"{
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": [],
        "rbac": {"assignments": [{
            "actor_id": "reviewer",
            "identity_type": "human",
            "capabilities": ["read", "edit", "execute", "manifest-write"],
            "scopes": ["default"]
        }]}
    }"#;
    let manifest: Manifest = serde_json::from_str(body).unwrap();
    let section = manifest.rbac.as_ref().unwrap();
    assert_eq!(section.assignments.len(), 1);
    assert_eq!(section.assignments[0].actor_id, "reviewer");
    assert_eq!(
        section.assignments[0].identity_type,
        Some(crate::IdentityType::Human)
    );
    assert_eq!(section.assignments[0].capabilities.len(), 4);
    assert_eq!(section.assignments[0].scopes, vec!["default".to_string()]);
    let written = serde_json::to_string(&manifest).unwrap();
    assert!(written.contains("\"rbac\""), "{written}");
    assert!(written.contains("manifest-write"), "{written}");
}

#[test]
fn manifest_without_rbac_parses_and_serializes_without_the_key() {
    let manifest: Manifest =
        serde_json::from_str(r#"{"schema_version":1,"scopes":[],"disposition_actor_ids":[]}"#)
            .unwrap();
    assert!(manifest.rbac.is_none());
    let written = serde_json::to_string(&manifest).unwrap();
    assert!(!written.contains("rbac"), "{written}");
}

#[test]
fn unknown_keys_inside_the_section_and_assignments_refuse() {
    let bad_section = r#"{"assignments": [], "wildcard": true}"#;
    let bad_assignment = r#"{"assignments": [{
        "actor_id": "x", "capabilities": ["read"], "scopes": ["default"], "expires": "soon"
    }]}"#;
    for body in [bad_section, bad_assignment] {
        let manifest_body = format!(
            r#"{{"schema_version":1,"scopes":[],"disposition_actor_ids":[],"rbac":{body}}}"#
        );
        let outcome = serde_json::from_str::<Manifest>(&manifest_body);
        let error = outcome.expect_err("unknown rbac keys must refuse");
        assert!(error.to_string().contains("unknown"), "{error}");
    }
}

#[test]
fn capability_strings_outside_the_closed_set_refuse() {
    let manifest_body = r#"{"schema_version":1,"scopes":[],"disposition_actor_ids":[],
        "rbac":{"assignments":[{"actor_id":"x","capabilities":["admin"],"scopes":["default"]}]}}"#;
    let outcome = serde_json::from_str::<Manifest>(manifest_body);
    let error = outcome.expect_err("unknown capability must refuse");
    assert!(error.to_string().contains("unknown variant"), "{error}");
}

#[test]
fn duplicate_actor_scope_pairs_and_empty_actor_ids_refuse() {
    let duplicated = section(vec![
        assignment("dev", &[Capability::Edit], &["default"]),
        assignment("dev", &[Capability::Edit], &["default"]),
    ]);
    let error = ensure_rbac_section_well_formed(&duplicated).unwrap_err();
    assert!(error.to_string().contains("duplicate"), "{error}");

    let repeated_scope = section(vec![assignment(
        "dev",
        &[Capability::Edit],
        &["default", "default"],
    )]);
    assert!(ensure_rbac_section_well_formed(&repeated_scope).is_err());

    let empty_actor = section(vec![assignment("", &[Capability::Read], &["default"])]);
    let error = ensure_rbac_section_well_formed(&empty_actor).unwrap_err();
    assert!(error.to_string().contains("actor_id"), "{error}");

    let good = section(vec![assignment("dev", &[Capability::Edit], &["default"])]);
    ensure_rbac_section_well_formed(&good).unwrap();
}

#[test]
fn ambiguity_law_refuses_nonempty_legacy_plus_rbac_with_the_fixed_golden() {
    let error = ensure_unambiguous_rbac(&["ben".to_string()], Some(&section(vec![]))).unwrap_err();
    assert_eq!(error.to_string(), AMBIGUOUS_MANIFEST_REFUSAL);
}

#[test]
fn ambiguity_law_admits_empty_legacy_beside_rbac_and_legacy_only() {
    ensure_unambiguous_rbac(&[], Some(&section(vec![]))).unwrap();
    ensure_unambiguous_rbac(&["ben".to_string()], None).unwrap();
    ensure_unambiguous_rbac(&[], None).unwrap();
}

#[test]
fn missing_claim_and_wrong_principal_refuse_with_distinct_goldens() {
    let grants = section(vec![assignment(
        "reviewer",
        &[Capability::Edit],
        &["default"],
    )]);
    let error = authorize(
        None,
        &grants,
        Capability::Edit,
        RbacResource::Scope(&scope_id("default")),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), MISSING_CLAIM_REFUSAL);

    let error = authorize(
        Some(&RbacClaim::new("intruder").unwrap()),
        &grants,
        Capability::Edit,
        RbacResource::Scope(&scope_id("default")),
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rbac: actor intruder does not hold capability edit on scope default"
    );
    assert_ne!(error.to_string(), MISSING_CLAIM_REFUSAL);
}

#[test]
fn a_grant_admits_its_own_scope_and_refuses_cross_scope() {
    let grants = section(vec![assignment(
        "reviewer",
        &[Capability::Edit],
        &["default"],
    )]);
    let claim = RbacClaim::new("reviewer").unwrap();
    authorize(
        Some(&claim),
        &grants,
        Capability::Edit,
        RbacResource::Scope(&scope_id("default")),
    )
    .unwrap();
    let error = authorize(
        Some(&claim),
        &grants,
        Capability::Edit,
        RbacResource::Scope(&scope_id("docs")),
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rbac: actor reviewer does not hold capability edit on scope docs"
    );
    let error = authorize(
        Some(&claim),
        &grants,
        Capability::Execute,
        RbacResource::Scope(&scope_id("default")),
    )
    .unwrap_err();
    assert!(error.to_string().contains("execute"), "{error}");
}

#[test]
fn repo_global_authority_requires_the_capability_on_every_scope() {
    let scopes = [scope_id("default"), scope_id("docs")];
    let covered = section(vec![assignment(
        "operator",
        &[Capability::ManifestWrite],
        &["default", "docs"],
    )]);
    let claim = RbacClaim::new("operator").unwrap();
    authorize(
        Some(&claim),
        &covered,
        Capability::ManifestWrite,
        RbacResource::RepoGlobal(&scopes),
    )
    .unwrap();

    let missing_docs = section(vec![assignment(
        "operator",
        &[Capability::ManifestWrite],
        &["default"],
    )]);
    let error = authorize(
        Some(&claim),
        &missing_docs,
        Capability::ManifestWrite,
        RbacResource::RepoGlobal(&scopes),
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rbac: actor operator does not hold capability manifest-write on scope docs"
    );
}

#[test]
fn ratification_demands_a_human_typed_assignment() {
    let assignments = vec![
        assignment("robot", &[Capability::Execute], &["default"]),
        {
            let mut agent = assignment("typed-agent", &[Capability::Execute], &["default"]);
            agent.identity_type = Some(crate::IdentityType::Agent);
            agent
        },
        {
            let mut human = assignment("ben", &[Capability::Execute], &["default"]);
            human.identity_type = Some(crate::IdentityType::Human);
            human
        },
    ];
    ensure_disposition_actor_is_human("ben", &assignments).unwrap();
    for actor in ["robot", "typed-agent", "stranger"] {
        let error = ensure_disposition_actor_is_human(actor, &assignments).unwrap_err();
        let message = error.to_string();
        assert!(
            message.starts_with("rbac: disposition actor ")
                && message.ends_with(RATIFICATION_REFUSAL_TAIL),
            "{message}"
        );
        assert!(message.contains(actor), "{message}");
    }
}

#[test]
fn the_legacy_manifest_fixture_shape_still_parses() {
    let manifest = json!({
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": ["reviewer"]
    });
    let manifest: Manifest = serde_json::from_value(manifest).unwrap();
    assert_eq!(manifest.disposition_actor_ids, vec!["reviewer".to_string()]);
    assert!(manifest.rbac.is_none());
}

#[test]
fn capability_wire_names_are_the_four_documented_words() {
    for (capability, word) in [
        (Capability::Read, "read"),
        (Capability::Edit, "edit"),
        (Capability::Execute, "execute"),
        (Capability::ManifestWrite, "manifest-write"),
    ] {
        assert_eq!(capability.as_str(), word);
        assert_eq!(
            serde_json::to_value(capability).unwrap(),
            serde_json::Value::String(word.to_string())
        );
        assert_eq!(Capability::parse(word).unwrap(), capability);
    }
    assert!(Capability::parse("own").is_err());
}

fn manifest_with(rbac: Option<RbacSection>, legacy: Vec<String>) -> Manifest {
    Manifest {
        schema_version: crate::SchemaVersion(1),
        scopes: vec![crate::Scope {
            id: scope_id("default"),
            path_prefix: RepoPathPrefix::new("."),
        }],
        disposition_actor_ids: legacy,
        rbac,
    }
}

#[test]
fn the_manifest_resolves_the_ratification_regime_from_the_section() {
    let legacy = manifest_with(None, vec!["ben".to_string()]);
    assert!(matches!(
        legacy.disposition_ratification(),
        crate::DispositionRatification::LegacyAllowlist(_)
    ));

    let rbac = manifest_with(Some(section(vec![])), Vec::new());
    assert!(matches!(
        rbac.disposition_ratification(),
        crate::DispositionRatification::RbacAssignments(_)
    ));
}
