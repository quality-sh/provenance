//! The Rust runner of the cross-SDK identity corpus (V3): retire and
//! reactivate, local-to-shared migration, and mixed-case keys, all
//! through kernel-authored documents.

use provenance_core::{Manifest, RepoPathPrefix, RequirementStatus, ScopeId, StableId};
use provenance_sdk::{operations, requirement, rule, spec, RequirementBuilder};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::{CreateRequirementInput, StateStore, TypedResourceKind};

fn repository() -> (tempfile::TempDir, camino::Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().canonicalize().unwrap()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    (dir, root)
}

fn apply(
    root: &camino::Utf8PathBuf,
    requirements: impl IntoIterator<Item = RequirementBuilder>,
) -> provenance_sdk::TypedSpecResult {
    let document = spec("share-links")
        .requirements(requirements)
        .build()
        .unwrap();
    operations::apply(
        Some(root.clone()),
        &ScopeId::new("default").unwrap(),
        document.materialize("spec://rust"),
    )
    .unwrap()
}

fn rule_resource(
    result: &provenance_sdk::TypedSpecResult,
    key: &str,
) -> provenance_store::state_store::ReconciledResource {
    result
        .resources
        .iter()
        .find(|resource| resource.kind == TypedResourceKind::Rule && resource.key == key)
        .unwrap()
        .clone()
}

#[test]
fn an_omitted_rule_retires_and_returns_under_its_id() {
    let (_dir, root) = repository();
    let sharing = || {
        requirement("sharing")
            .statement("Users can securely share documentation")
            .rules([rule("expiry").statement("Share links expire within 30 days")])
    };
    let first = apply(&root, [sharing()]);
    let expiry = rule_resource(&first, "expiry");

    let without = apply(
        &root,
        [requirement("sharing").statement("Users can securely share documentation")],
    );
    assert_eq!(without.retired, 1);

    let restored = apply(&root, [sharing()]);
    let returned = rule_resource(&restored, "expiry");
    assert_eq!(
        returned.id, expiry.id,
        "reactivation keeps the canonical id"
    );
}

#[test]
fn a_rule_migrates_from_local_to_shared_and_keeps_its_id() {
    let (_dir, root) = repository();
    let local = apply(
        &root,
        [requirement("sharing")
            .statement("Users can securely share documentation")
            .rules([rule("audit").statement("Share-link access is audited")])],
    );
    let before = rule_resource(&local, "audit");
    assert_eq!(
        before.address.segments(),
        ["share-links", "requirement", "sharing", "rule", "audit"]
    );

    let shared = apply(
        &root,
        [
            requirement("sharing")
                .statement("Users can securely share documentation")
                .rules([rule("audit")
                    .statement("Share-link access is audited")
                    .requirements(["retention"])]),
            requirement("retention").statement("Share records are retained"),
        ],
    );
    let after = rule_resource(&shared, "audit");
    assert_eq!(after.address.segments(), ["share-links", "rule", "audit"]);
    assert_eq!(after.id, before.id, "migration keeps the canonical id");
}

#[test]
fn mixed_case_keys_keep_distinct_stable_identities() {
    let (_dir, root) = repository();
    let result = apply(
        &root,
        [
            requirement("Sharing").statement("Statement one"),
            requirement("sharing").statement("Statement two"),
        ],
    );
    let ids = result
        .resources
        .iter()
        .map(|resource| resource.id.as_str().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 2);

    let again = apply(
        &root,
        [
            requirement("Sharing").statement("Statement one"),
            requirement("sharing").statement("Statement two"),
        ],
    );
    assert_eq!(again.unchanged, 2);
}

#[test]
fn a_kernel_authored_requirement_adopts_one_exact_unowned_identity() {
    let (_dir, root) = repository();
    let scope = ScopeId::new("default").unwrap();
    let store = StateStore::new(ProvenanceLayout::new(root.clone()));
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_existing").unwrap(),
            statement: "The canonical Requirement keeps its identity".to_string(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let document = spec("migration")
        .requirements([requirement("canonical")
            .adopt_unowned("req_existing")
            .statement("The canonical Requirement keeps its identity")])
        .build()
        .unwrap()
        .materialize("spec://rust/migration");

    let preview = operations::plan(Some(root.clone()), &scope, document.clone()).unwrap();
    assert_eq!(
        (
            preview.reconciliation.created,
            preview.reconciliation.conflicts
        ),
        (0, 0)
    );
    let applied = operations::apply(Some(root.clone()), &scope, document.clone()).unwrap();
    assert_eq!(applied.resources[0].id.as_str(), "req_existing");
    let replay = operations::plan(Some(root), &scope, document).unwrap();
    assert_eq!(replay.reconciliation.unchanged, 1);
}
