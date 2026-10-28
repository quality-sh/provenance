//! The opened repository: open, discover, settings once, typed refusals,
//! the declared scope, and the scope the handle selects.

use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId};
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_store::repository::{OpenFailure, Repository, ScopeMismatch};
use provenance_store::settings::SettingsError;
use serde_json::json;

fn two_scope_repository() -> (tempfile::TempDir, ProvenanceLayout) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let mut manifest =
        Manifest::default_with_scope(ScopeId::new("a").unwrap(), RepoPathPrefix::new("a"));
    manifest.scopes.push(Scope {
        id: ScopeId::new("b").unwrap(),
        path_prefix: RepoPathPrefix::new("b"),
    });
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    (dir, layout)
}

fn settings_path(layout: &ProvenanceLayout) -> camino::Utf8PathBuf {
    layout.provenance_dir().join("settings.json")
}

#[test]
fn open_canonicalizes_the_root_and_selects_the_declared_scope() {
    let (dir, _layout) = two_scope_repository();
    let repository = Repository::open(dir.path(), "b").unwrap();
    assert_eq!(repository.scope().as_str(), "b");
    assert_eq!(
        repository.working_tree(),
        camino::Utf8PathBuf::from_path_buf(dir.path().canonicalize().unwrap()).unwrap()
    );
    assert_eq!(repository.identity().scope(), repository.scope());
    assert_eq!(repository.manifest().unwrap().scopes.len(), 2);
}

#[test]
#[verifies("rule_graph_report_refuses_undeclared_scope", examples)]
fn open_refuses_an_undeclared_scope_and_names_the_declared_ones() {
    let (dir, _layout) = two_scope_repository();
    let error = Repository::open(dir.path(), "c").unwrap_err();
    assert!(matches!(
        &error,
        OpenFailure::UnknownScope { selected, declared }
            if selected == "c" && declared == &["a".to_owned(), "b".to_owned()]
    ));
    assert_eq!(
        error.to_string(),
        "unknown scope `c`; declared scopes: a, b"
    );
}

#[test]
fn open_refuses_an_absent_root_and_an_uninitialized_root() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Repository::open(dir.path().join("missing"), "default"),
        Err(OpenFailure::InvalidRoot { .. })
    ));
    let error = Repository::open(dir.path(), "default").unwrap_err();
    assert!(matches!(error, OpenFailure::NotInitialized), "{error}");
    assert!(error.to_string().contains("not initialized"));
}

#[test]
#[verifies("rule_repository_reads_settings_once_at_open", examples)]
fn settings_are_read_once_at_open_and_an_invalid_file_refuses_the_open() {
    let (dir, layout) = two_scope_repository();
    std::fs::write(
        settings_path(&layout),
        json!({"read": {"freshness_policy": "refuse_stale", "scan_limit": 3}}).to_string(),
    )
    .unwrap();
    let repository = Repository::open(dir.path(), "a").unwrap();
    let policy = ReadPolicy::resolve(repository.settings(), None);
    assert_eq!(policy.freshness, FreshnessPolicy::RefuseStale);
    assert_eq!(policy.scan_limit, 3);

    // A later edit does not reach the open handle; a new open reads it.
    std::fs::write(
        settings_path(&layout),
        json!({"read": {"freshness_policy": "annotate_only"}}).to_string(),
    )
    .unwrap();
    let unchanged = ReadPolicy::resolve(repository.settings(), None);
    assert_eq!(unchanged.freshness, FreshnessPolicy::RefuseStale);
    assert_eq!(unchanged.scan_limit, 3);
    let reopened = Repository::open(dir.path(), "a").unwrap();
    assert_eq!(
        ReadPolicy::resolve(reopened.settings(), None).freshness,
        FreshnessPolicy::AnnotateOnly
    );

    std::fs::write(
        settings_path(&layout),
        json!({"read": {"freshness_policy": "fast"}}).to_string(),
    )
    .unwrap();
    let error = Repository::open(dir.path(), "a").unwrap_err();
    let OpenFailure::Settings(SettingsError::Invalid { path, key, problem }) = &error else {
        panic!("expected a typed settings refusal: {error}");
    };
    assert_eq!(path, &settings_path(&layout));
    assert_eq!(key, "read.freshness_policy");
    assert!(problem.contains("catch_up, annotate_only, refuse_stale"));
}

#[test]
#[verifies("rule_operation_scope_is_the_opened_scope", examples)]
fn ensure_selected_refuses_a_mismatch() {
    let (dir, _layout) = two_scope_repository();
    let repository = Repository::open(dir.path(), "a").unwrap();
    assert_eq!(
        repository.ensure_selected(&ScopeId::new("a").unwrap()),
        Ok(())
    );
    assert_eq!(
        repository.ensure_selected(&ScopeId::new("b").unwrap()),
        Err(ScopeMismatch {
            selected: ScopeId::new("a").unwrap(),
            requested: ScopeId::new("b").unwrap(),
        })
    );
}

#[test]
#[verifies("rule_operation_scope_is_the_opened_scope", examples)]
fn apply_through_the_handle_lands_in_the_selected_scope() {
    let (dir, layout) = two_scope_repository();
    let repository = Repository::open(dir.path(), "b").unwrap();
    let input: provenance_store::state_store::TypedSpecInput = serde_json::from_value(json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "spec": "scoped",
        "declared_by": "spec://rust",
        "requirements": [{"key": "sharing", "statement": "Users can share documentation"}]
    }))
    .unwrap();
    let result = provenance_store::operations::apply(&repository, input).unwrap();
    assert_eq!(result.created, 1);
    assert!(layout.scopes_dir().join("b").is_dir());
    assert!(!layout.scopes_dir().join("a").exists());
    let store = repository.store();
    assert_eq!(
        store
            .list_requirements(&ScopeId::new("b").unwrap())
            .unwrap()
            .len(),
        1
    );
    assert!(store
        .list_requirements(&ScopeId::new("a").unwrap())
        .unwrap()
        .is_empty());
}

#[test]
#[verifies("rule_sdk_project_discovery", examples)]
fn discover_resolves_an_explicit_root_without_reading_the_current_directory() {
    let (dir, _layout) = two_scope_repository();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let repository = Repository::discover(Some(root), "a").unwrap();
    assert_eq!(repository.scope().as_str(), "a");
    assert_eq!(
        repository.working_tree(),
        camino::Utf8PathBuf::from_path_buf(dir.path().canonicalize().unwrap()).unwrap()
    );
}
