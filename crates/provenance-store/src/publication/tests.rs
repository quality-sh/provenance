use super::*;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;

#[test]
fn concurrent_first_access_creates_publication_lock_directories_idempotently() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads = (0..8)
        .map(|_| {
            let layout = ProvenanceLayout::new(root.clone());
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                with_repository_publication(&layout, || Ok(()))
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        thread.join().unwrap().unwrap();
    }
}

#[test]
fn source_edit_recovery_runs_before_import_recovery() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(layout.import_transactions_dir()).unwrap();
    std::fs::write(layout.source_edit_marker_path(), "{").unwrap();
    let transaction = layout.import_transactions_dir().join("completed");
    std::fs::create_dir(&transaction).unwrap();
    write_publication_marker(&layout, &transaction, PublicationPhase::Published).unwrap();
    std::fs::remove_dir(transaction).unwrap();

    let error = with_repository_publication(&layout, || Ok(())).unwrap_err();

    assert!(error.to_string().contains("EOF"), "{error:#}");
    assert!(layout.publication_marker_path().exists());
}

#[test]
#[verifies("rule_recovery_stays_in_cache", examples)]
fn recovery_rejects_traversal_outside_import_transactions() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(layout.import_transactions_dir()).unwrap();
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("sentinel"), "keep").unwrap();
    std::fs::write(
        layout.publication_marker_path(),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": layout.import_transactions_dir().join("../../../outside"),
            "phase": "published"
        }))
        .unwrap(),
    )
    .unwrap();

    let error = recover_pending_publication(&layout)
        .unwrap_err()
        .to_string();

    assert!(error.contains("outside the repository cache"), "{error}");
    assert!(outside.join("sentinel").is_file());
}

#[cfg(unix)]
#[test]
#[verifies("rule_recovery_stays_in_cache", examples)]
fn recovery_rejects_symlinked_import_transactions_outside_repository() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    let outside = Utf8PathBuf::from_path_buf(directory.path().join("outside")).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(layout.cache_dir()).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, layout.import_transactions_dir()).unwrap();
    let transaction = layout.import_transactions_dir().join("Documents");
    std::fs::create_dir_all(&transaction).unwrap();
    std::fs::write(transaction.join("sentinel"), "keep").unwrap();
    std::fs::write(
        layout.publication_marker_path(),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": transaction,
            "phase": "published"
        }))
        .unwrap(),
    )
    .unwrap();

    let error = recover_pending_publication(&layout)
        .unwrap_err()
        .to_string();

    assert!(error.contains("symlink component"), "{error}");
    assert!(outside.join("Documents/sentinel").is_file());
}

#[cfg(unix)]
#[test]
#[verifies("rule_recovery_stays_in_cache", examples)]
fn recovery_rejects_symlinked_import_transactions_inside_repository() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    let tracked = root.join("tracked");
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(layout.cache_dir()).unwrap();
    std::fs::create_dir_all(tracked.join("transaction")).unwrap();
    std::fs::write(tracked.join("transaction/sentinel"), "keep").unwrap();
    std::os::unix::fs::symlink(&tracked, layout.import_transactions_dir()).unwrap();
    let transaction = layout.import_transactions_dir().join("transaction");
    std::fs::write(
        layout.publication_marker_path(),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": transaction,
            "phase": "published"
        }))
        .unwrap(),
    )
    .unwrap();

    let error = recover_pending_publication(&layout)
        .unwrap_err()
        .to_string();

    assert!(error.contains("symlink component"), "{error}");
    assert!(tracked.join("transaction/sentinel").is_file());
}

#[cfg(unix)]
#[test]
#[verifies("rule_recovery_stays_in_cache", examples)]
fn recovery_rejects_a_symlink_component_even_when_the_written_path_resolves_inside_cache() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(layout.import_transactions_dir().join("detour")).unwrap();
    let transaction = layout.import_transactions_dir().join("interrupted");
    std::fs::create_dir(&transaction).unwrap();
    std::fs::write(transaction.join("sentinel"), "keep").unwrap();
    let link = layout.import_transactions_dir().join("link");
    std::os::unix::fs::symlink("detour", &link).unwrap();
    let written_transaction = link.join("../interrupted");
    std::fs::write(
        layout.publication_marker_path(),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": written_transaction,
            "phase": "published"
        }))
        .unwrap(),
    )
    .unwrap();

    let error = recover_pending_publication(&layout)
        .unwrap_err()
        .to_string();

    assert!(error.contains("symlink component"), "{error}");
    assert_eq!(
        std::fs::read_to_string(transaction.join("sentinel")).unwrap(),
        "keep"
    );
    assert!(layout.publication_marker_path().is_file());
}

#[test]
#[verifies("rule_recovery_stays_in_cache", examples)]
fn recovery_clears_published_marker_when_transaction_is_missing() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(layout.import_transactions_dir()).unwrap();
    let transaction = layout.import_transactions_dir().join("completed");
    std::fs::write(
        layout.publication_marker_path(),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": transaction,
            "phase": "published"
        }))
        .unwrap(),
    )
    .unwrap();

    recover_pending_publication(&layout).unwrap();

    assert!(layout.state_dir().is_dir());
    assert!(!layout.publication_marker_path().exists());
}

#[test]
#[verifies("rule_recovery_stays_in_cache", examples)]
fn recovery_rejects_a_transaction_that_is_a_regular_file() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(layout.import_transactions_dir()).unwrap();
    let transaction = layout.import_transactions_dir().join("interrupted");
    std::fs::write(&transaction, "not a directory").unwrap();
    std::fs::write(
        layout.publication_marker_path(),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": transaction,
            "phase": "backup_created"
        }))
        .unwrap(),
    )
    .unwrap();

    let error = recover_pending_publication(&layout)
        .unwrap_err()
        .to_string();

    assert!(error.contains("is not a directory"), "{error}");
    assert_eq!(
        std::fs::read_to_string(&transaction).unwrap(),
        "not a directory"
    );
    assert!(layout.publication_marker_path().is_file());
}

/// The rule assumes the cache itself is a real directory and leaves proving it
/// to `create_real_directory`. This checks the two halves compose where they
/// meet: with a symlink standing in for the cache, the entry point every
/// caller goes through refuses before recovery reads a marker or touches the
/// tree the link points at.
#[cfg(unix)]
#[test]
#[verifies("rule_recovery_stays_in_cache", conformance)]
fn a_symlinked_cache_is_refused_before_recovery_runs() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    let elsewhere = Utf8PathBuf::from_path_buf(directory.path().join("elsewhere")).unwrap();
    let layout = ProvenanceLayout::new(root);
    let transaction = elsewhere.join("import-transactions/interrupted");
    std::fs::create_dir_all(layout.provenance_dir()).unwrap();
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(&transaction).unwrap();
    std::fs::write(transaction.join("sentinel"), "keep").unwrap();
    std::os::unix::fs::symlink(&elsewhere, layout.cache_dir()).unwrap();
    std::fs::write(
        layout.publication_marker_path(),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": transaction,
            "phase": "backup_created"
        }))
        .unwrap(),
    )
    .unwrap();

    let mut operation_ran = false;
    let error = with_repository_publication(&layout, || {
        operation_ran = true;
        Ok(())
    })
    .unwrap_err()
    .to_string();

    assert!(error.contains("symlink component"), "{error}");
    assert!(!operation_ran);
    assert_eq!(
        std::fs::read_to_string(transaction.join("sentinel")).unwrap(),
        "keep"
    );
}

#[test]
fn publication_lock_marker_is_cleared_after_panic() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::create_dir_all(layout.import_transactions_dir()).unwrap();

    let panic = std::panic::catch_unwind(|| {
        with_repository_publication(&layout, || -> anyhow::Result<()> {
            panic!("publication interrupted");
        })
        .unwrap();
    });
    assert!(panic.is_err());
    let key = publication_lock_identity(&layout).unwrap();
    assert!(!HELD_LOCKS.with(|locks| locks.borrow().contains(&key)));

    let transaction = layout.import_transactions_dir().join("completed");
    std::fs::create_dir(&transaction).unwrap();
    write_publication_marker(&layout, &transaction, PublicationPhase::Published).unwrap();
    std::fs::remove_dir(transaction).unwrap();

    with_repository_publication(&layout, || Ok(())).unwrap();

    assert!(!layout.publication_marker_path().exists());
}

// Implementation aid: checks nested publication sections through path aliases.
// rule_store_under_guard_takes_no_second_lock covers guarded store reads instead.
#[cfg(unix)]
#[test]
fn nested_publication_through_a_repository_alias_completes() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    let alias = temporary.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let layout = ProvenanceLayout::new(root.to_str().unwrap());
    let alias = ProvenanceLayout::new(alias.to_str().unwrap());
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result =
            with_repository_publication(&layout, || with_repository_publication(&alias, || Ok(())));
        sender.send(result).unwrap();
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("a nested publication through an alias must complete")
        .unwrap();
}

// Implementation aid: checks alias identity during validation, without a guarded store.
#[cfg(unix)]
#[test]
fn read_only_validation_through_an_alias_does_not_publish() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    let alias = temporary.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let layout = ProvenanceLayout::new(root.to_str().unwrap());
    let alias_path = alias;
    let alias = ProvenanceLayout::new(alias_path.to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&provenance_core::Manifest::default_with_scope(
            provenance_core::ScopeId::new("default").unwrap(),
            provenance_core::RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    with_read_only_validation(&layout, || {
        with_read_only_validation(&alias, || Ok(()))?;
        with_repository_publication(&alias, || Ok(()))?;
        let repository = crate::repository::Repository::open(&alias_path, "default")?;
        let _guard = runtime.block_on(publication_guard(&repository))?;
        assert!(!layout.publication_lock_path().exists());
        assert!(!layout.import_transactions_dir().exists());
        Ok(())
    })
    .unwrap();
}

// Implementation aid: init validates planned state before its repository exists.
#[test]
fn read_only_validation_accepts_a_planned_repository() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("planned/repository");
    let layout = ProvenanceLayout::new(root.to_str().unwrap());
    with_read_only_validation(&layout, || {
        with_repository_publication(&layout, || Ok(()))?;
        Ok(())
    })
    .unwrap();
    assert!(!root.exists());
}

/// A repository with one Source, and a copy of its state with no cache.
fn record_kind_repositories() -> (tempfile::TempDir, ProvenanceLayout, ProvenanceLayout) {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let live = ProvenanceLayout::new(root.join("live"));
    crate::cache::tests::fixtures::initialized(live.root().as_std_path());
    let scope = provenance_core::ScopeId::new("default").unwrap();
    crate::cache::tests::fixtures::create_source(
        &crate::test_support::store(&live),
        &scope,
        "source_help",
    );
    let copy = ProvenanceLayout::new(root.join("copy"));
    staged::copy_directory(&live.state_dir(), &copy.state_dir()).unwrap();
    (directory, live, copy)
}

#[test]
#[verifies("rule_read_only_section_stays_inside_the_store", examples)]
fn read_only_record_kinds_answer_while_another_handle_holds_the_lock() {
    let (_directory, live, _copy) = record_kind_repositories();
    let _held = guard::LockedPublicationFile::acquire(&live.publication_lock_path()).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = live.clone();
    std::thread::spawn(move || {
        let scope = provenance_core::ScopeId::new("default").unwrap();
        sender
            .send(read_only_record_kinds(&reader, &scope, "source_help").unwrap())
            .unwrap();
    });
    let kinds = receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the read takes no publication lock");
    assert_eq!(kinds, vec![provenance_core::NodeType::Source]);
}

#[test]
#[verifies("rule_read_only_section_stays_inside_the_store", examples)]
fn read_only_record_kinds_create_no_path() {
    let (_directory, _live, copy) = record_kind_repositories();
    let scope = provenance_core::ScopeId::new("default").unwrap();
    assert_eq!(
        read_only_record_kinds(&copy, &scope, "source_help").unwrap(),
        vec![provenance_core::NodeType::Source]
    );
    assert_eq!(
        read_only_record_kinds(&copy, &scope, "source_missing").unwrap(),
        Vec::new()
    );
    assert!(!copy.cache_dir().exists(), "the read creates no cache");
}
