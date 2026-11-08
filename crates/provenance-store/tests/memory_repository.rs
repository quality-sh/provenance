//! A repository over the memory adapter writes records with no file access.
//!
//! The repository is rooted at an empty directory. Each write set reaches
//! the persistence of the adapter, and the directory stays empty. The state
//! starts with the manifest, one Requirement and one Rule. Live writes make
//! and change a verification binding of the Rule; stages make and change a
//! Requirement and a Rule and import a scope, each as one whole set.
//!
//! The tests run with and without the `host` feature; without it the store
//! has no code that can reach the local disk.

#[path = "support/memory.rs"]
mod memory;

use memory::{id, imported, initial_state, memory_repository, scope, MemoryRepository};
use provenance_core::{
    RequirementStatus, RuleSeverity, RuleStatus, SourceType, StableId, VerificationMethod,
};
use provenance_macros::verifies;
#[cfg(feature = "host")]
use provenance_store::operations::files::RepositoryFiles;
#[cfg(feature = "host")]
use provenance_store::publication::with_staged_state_and_source_edit;
use provenance_store::repository::Repository;
use provenance_store::state_store::{
    CreateRequirementInput, CreateRuleInput, MaterializeVerificationBindingInput, ScopeShards,
    UpdateRequirementInput, UpdateRuleInput,
};
use provenance_store::storage::{Atomicity, PublishFailure, StateKey, StateStorage, WriteSet};

/// The binding of the Rule with the key `share-link-expiry` in `file`.
fn binding(file: &str) -> MaterializeVerificationBindingInput {
    MaterializeVerificationBindingInput {
        scope_id: scope(),
        rule_id: id("rule_expiry"),
        key: "share-link-expiry".into(),
        method: VerificationMethod::Examples,
        declared_by: "test://typescript".into(),
        file: file.into(),
        symbol: None,
    }
}

/// Makes or changes the binding of the Rule with the key `share-link-expiry`.
fn bind(repository: &Repository, file: &str) {
    repository
        .store()
        .materialize_verification_binding(binding(file))
        .unwrap();
}

#[test]
#[verifies("rule_engine_runs_on_any_storage", examples)]
fn a_repository_over_memory_writes_live_records_with_no_file() {
    let memory = memory_repository();
    bind(&memory.repository, "tests/share-links.test.ts");
    bind(&memory.repository, "tests/expiry.test.ts");

    memory.assert_empty();
    let view = memory.repository.view().unwrap();
    assert_eq!(view.list_requirements(&scope()).unwrap().len(), 1);
    assert_eq!(view.list_rules(&scope()).unwrap().len(), 1);
    let bindings = view.list_verification_bindings(&scope()).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].file.as_str(), "tests/expiry.test.ts");
    assert_eq!(memory.recorded.0.lock().unwrap().len(), 2);
    assert_eq!(
        memory.replayed().revision(),
        memory.storage.read_view().unwrap().revision()
    );
}

/// Runs `stage` and asserts that it gave the persistence one whole set on
/// the revision before it.
fn assert_one_whole_set(memory: &MemoryRepository, stage: impl FnOnce(&Repository)) {
    let before = memory.storage.read_view().unwrap();
    let count = memory.recorded.0.lock().unwrap().len();
    stage(&memory.repository);
    let recorded = memory.recorded.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), count + 1, "the stage gave one write set");
    let writes = recorded.last().unwrap();
    assert_eq!(writes.atomicity(), Atomicity::WholeSet);
    assert_eq!(writes.base(), Some(before.revision()));
    assert!(writes.key_count() > 0);
}

fn requirement_input(id: &str) -> CreateRequirementInput {
    CreateRequirementInput {
        scope_id: scope(),
        id: StableId::new(id).unwrap(),
        statement: "Share links have an owner".into(),
        description: None,
        status: RequirementStatus::Active,
        refines: Vec::new(),
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn rule_input(id: &str, requirement: &str) -> CreateRuleInput {
    CreateRuleInput {
        archived_in_commit: None,
        scope_id: scope(),
        id: StableId::new(id).unwrap(),
        name: None,
        description: None,
        requirement_ids: vec![StableId::new(requirement).unwrap()],
        resolution_ids: Vec::new(),
        statement: "Share links have an owner".into(),
        status: RuleStatus::Active,
        severity: RuleSeverity::Medium,
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn owner_description() -> UpdateRequirementInput {
    UpdateRequirementInput {
        scope_id: scope(),
        id: StableId::new("req_owner").unwrap(),
        expected_etag: None,
        declared_by: None,
        statement: None,
        description: Some("Each share link names its owner.".into()),
        status: None,
        archived_in_commit: None,
        clear_fields: Vec::new(),
    }
}

fn owner_rule_name() -> UpdateRuleInput {
    UpdateRuleInput {
        scope_id: scope(),
        id: StableId::new("rule_owner").unwrap(),
        expected_etag: None,
        declared_by: None,
        name: Some("Share link owner".into()),
        description: None,
        statement: None,
        status: None,
        severity: Some(RuleSeverity::High),
        source_document: None,
        source_section: None,
        archived_in_commit: None,
        requirement_ids: None,
        resolution_ids: None,
        clear_fields: Vec::new(),
    }
}

fn imported_source() -> provenance_core::Source {
    provenance_core::Source {
        created: None,
        updated: None,
        scope_id: imported(),
        id: StableId::new("source_policy").unwrap(),
        declared_by: None,
        declaration_address: None,
        name: "Sharing policy".into(),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

#[test]
#[verifies("rule_engine_runs_on_any_storage", examples)]
#[verifies("rule_stage_lands_as_one_whole_set", examples)]
fn a_repository_over_memory_makes_changes_and_imports_records_in_stages() {
    let memory = memory_repository();
    assert_one_whole_set(&memory, |repository| {
        repository
            .store()
            .create_requirement(requirement_input("req_owner"))
            .unwrap();
    });
    assert_one_whole_set(&memory, |repository| {
        repository
            .store()
            .create_rule(rule_input("rule_owner", "req_owner"))
            .unwrap();
    });
    assert_one_whole_set(&memory, |repository| {
        repository
            .store()
            .update_requirement(owner_description())
            .unwrap();
    });
    assert_one_whole_set(&memory, |repository| {
        repository.store().update_rule(owner_rule_name()).unwrap();
    });
    let source = imported_source();
    assert_one_whole_set(&memory, |repository| {
        repository
            .stage(false, None, |staged| {
                staged.import_scope(
                    &imported(),
                    &ScopeShards {
                        sources: std::slice::from_ref(&source),
                        ..ScopeShards::default()
                    },
                )
            })
            .unwrap();
    });
    let before_dry_run = memory.storage.read_view().unwrap();
    memory
        .repository
        .stage(true, None, |staged| {
            staged.import_scope(&imported(), &ScopeShards::default())
        })
        .unwrap();
    assert_eq!(memory.storage.read_view().unwrap(), before_dry_run);

    memory.assert_empty();
    let view = memory.repository.view().unwrap();
    let requirement = view
        .list_requirements(&scope())
        .unwrap()
        .into_iter()
        .find(|requirement| requirement.id.as_str() == "req_owner")
        .unwrap();
    assert_eq!(
        requirement.description.as_deref(),
        Some("Each share link names its owner.")
    );
    let rule = view
        .list_rules(&scope())
        .unwrap()
        .into_iter()
        .find(|rule| rule.id.as_str() == "rule_owner")
        .unwrap();
    assert_eq!(rule.severity, RuleSeverity::High);
    assert_eq!(view.list_sources(&imported()).unwrap().len(), 1);
    assert_eq!(
        memory.replayed().revision(),
        memory.storage.read_view().unwrap().revision()
    );
}

/// Implementation aid: no proposed Rule of the plan covers the section view
/// after a refused write. This test shows that a change outside the engine
/// refuses one write of a section and that the next write of the same
/// section reads the changed state and lands.
#[test]
fn a_section_reads_the_state_again_after_a_moved_base() {
    let memory = memory_repository();
    let store = memory.repository.store();
    store
        .with_repository_publication(|section| {
            section.materialize_verification_binding(binding("tests/first.test.ts"))?;

            let manifest = StateKey::new("manifest.json").unwrap();
            let mut bytes = initial_state().get(&manifest).unwrap().to_vec();
            bytes.push(b'\n');
            memory
                .storage
                .publish(WriteSet::new(Atomicity::PerKey).put(manifest, bytes))
                .unwrap();

            let refused = section
                .materialize_verification_binding(binding("tests/second.test.ts"))
                .unwrap_err();
            assert!(
                matches!(
                    refused.downcast_ref::<PublishFailure>(),
                    Some(PublishFailure::BaseMoved { .. })
                ),
                "the write after the outside change is refused: {refused:#}"
            );
            section.materialize_verification_binding(binding("tests/third.test.ts"))?;
            Ok(())
        })
        .unwrap();

    let bindings = memory
        .repository
        .view()
        .unwrap()
        .list_verification_bindings(&scope())
        .unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].file.as_str(), "tests/third.test.ts");
}

// A source edit changes a working-tree file, so it is a host operation.
#[cfg(feature = "host")]
#[test]
#[verifies("rule_engine_runs_on_any_storage", examples)]
fn a_source_edit_on_a_memory_repository_needs_the_local_adapter() {
    let memory = memory_repository();
    let root = camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(memory.root.path()).unwrap())
        .unwrap();
    std::fs::write(root.join("source.txt"), b"before\n").unwrap();
    let held = RepositoryFiles::open(&root)
        .unwrap()
        .read_bounded(camino::Utf8Path::new("source.txt"), 1024)
        .unwrap();
    let before = memory.storage.read_view().unwrap();

    let refused = memory
        .repository
        .store()
        .with_repository_publication(|section| {
            with_staged_state_and_source_edit(section, held, b"after\n", |_| Ok(()))
        })
        .unwrap_err();

    assert!(
        refused
            .to_string()
            .contains("needs the local storage adapter"),
        "the refusal names the adapter: {refused:#}"
    );
    assert!(refused.downcast_ref::<PublishFailure>().is_none());
    assert_eq!(memory.storage.read_view().unwrap(), before);
    assert!(memory.recorded.0.lock().unwrap().is_empty());
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), b"before\n");
}
