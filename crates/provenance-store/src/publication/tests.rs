use super::read_only::tests::{record_kind_repositories, scope};
use super::*;
use provenance_core::NodeType;
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
                with_repository_publication(&layout, |_| Ok(()))
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

    let error = with_repository_publication(&layout, |_| Ok(())).unwrap_err();

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
    let error = with_repository_publication(&layout, |_| {
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
        with_repository_publication(&layout, |_| -> anyhow::Result<()> {
            panic!("publication interrupted");
        })
        .unwrap();
    });
    assert!(panic.is_err());
    let key = publication_lock_identity(&layout).unwrap();
    refuse_if_held(&key, HolderKind::Live, layout.root()).expect("the panic left no holder");

    let transaction = layout.import_transactions_dir().join("completed");
    std::fs::create_dir(&transaction).unwrap();
    write_publication_marker(&layout, &transaction, PublicationPhase::Published).unwrap();
    std::fs::remove_dir(transaction).unwrap();

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert!(!layout.publication_marker_path().exists());
}

#[cfg(unix)]
#[test]
#[verifies("rule_publication_section_is_not_entered_twice", examples)]
fn a_nested_section_completes_on_the_held_store_and_is_refused_through_an_alias() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    let alias = temporary.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let layout = ProvenanceLayout::new(root.to_str().unwrap());
    let alias = ProvenanceLayout::new(alias.to_str().unwrap());
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = with_repository_publication(&layout, |held| {
            held.with_repository_publication(|_| Ok(()))?;
            let fresh = crate::state_store::StateStore::new(layout.clone());
            let refusals = [
                with_repository_publication(&alias, |_| Ok(())).unwrap_err(),
                fresh.with_repository_publication(|_| Ok(())).unwrap_err(),
            ];
            Ok(refusals.map(|error| error.is::<PublicationHeld>()))
        });
        sender.send(result).unwrap();
    });
    let refused = receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("a nested section answers without a wait")
        .unwrap();
    assert_eq!(refused, [true, true]);
}

/// The probe starts the host read while the publication holds its lock with the live state
/// moved away, and does not wait for it. The open and the manifest read wait for the publication.
#[test]
#[verifies("rule_preparation_reads_published_state", examples)]
fn a_repository_read_in_the_publication_gap_waits_for_the_publication() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest = provenance_core::Manifest::default_with_scope(
        provenance_core::ScopeId::new("default").unwrap(),
        provenance_core::RepoPathPrefix::new("."),
    );
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let reader = std::rc::Rc::new(std::cell::RefCell::new(None));
    crate::test_probes::arm("state_after_backup_rename", {
        let reader = reader.clone();
        move || {
            let root = root.clone();
            reader.replace(Some(std::thread::spawn(move || {
                crate::repository::Repository::open(root.as_std_path(), "default")?.manifest()
            })));
            Ok(())
        }
    });
    let mut published = manifest;
    published.disposition_actor_ids = vec!["actor_after".to_owned()];
    with_repository_publication(&layout, |held| {
        with_staged_state(held, false, |staged| {
            std::fs::write(staged.manifest_path(), serde_json::to_vec(&published)?)?;
            Ok(())
        })
    })
    .unwrap();
    crate::test_probes::disarm("state_after_backup_rename");
    let reader = reader.take().expect("the publication reached the gap");
    assert_eq!(reader.join().unwrap().unwrap(), published);
}

/// The probe reads the record kinds on another thread while the
/// publication has moved the live state to its backup. The read takes no
/// lock, so it does not wait, and it finds no record.
#[test]
#[verifies("rule_read_only_section_stays_inside_the_store", examples)]
fn read_only_record_kinds_in_the_publication_swap_find_no_record() {
    let (_directory, live, _copy) = record_kind_repositories();
    let answer = std::rc::Rc::new(std::cell::RefCell::new(None));
    crate::test_probes::arm("state_after_backup_rename", {
        let answer = answer.clone();
        let live = live.clone();
        move || {
            let live = live.clone();
            let reader =
                std::thread::spawn(move || read_only_record_kinds(&live, &scope(), "source_help"));
            answer.replace(Some(reader.join().expect("the reader does not panic")));
            Ok(())
        }
    });
    let published = with_repository_publication(&live, |held| {
        with_staged_state(held, false, |_staged| Ok(()))
    });
    crate::test_probes::disarm("state_after_backup_rename");
    published.unwrap();
    let answer = answer.take().expect("the publication reached the swap");
    assert_eq!(answer.unwrap(), Vec::<NodeType>::new());
    assert_eq!(
        read_only_record_kinds(&live, &scope(), "source_help").unwrap(),
        vec![NodeType::Source],
        "after the publication the record is found again"
    );
}

#[test]
#[verifies("rule_read_only_store_refuses_writes", examples)]
fn a_read_only_write_is_refused_and_creates_no_path() {
    let (_directory, _live, copy) = record_kind_repositories();
    let shard = crate::shards::path_for(&copy, &scope(), NodeType::Source);
    let before = std::fs::read(&shard).unwrap();
    let imported = provenance_core::ScopeId::new("imported").unwrap();
    with_read_only_validation(&copy, |store| {
        assert!(store
            .mutate_jsonl_records(&shard, |records: &mut Vec<serde_json::Value>| {
                records.clear();
                Ok(())
            })
            .is_err());
        assert!(store
            .mutate_jsonl_records_if(&shard, |_: &mut Vec<serde_json::Value>| Ok(true))
            .is_err());
        assert!(store
            .permitting(&[shard.as_path()], "*", |_| Ok(()))
            .is_err());
        assert!(with_staged_state(store, false, |_| Ok(())).is_err());
        let mut sources = store.list_sources(&scope())?;
        sources[0].id = provenance_core::StableId::new("source_not_in_repository")?;
        let refusals = [
            store
                .import_scope(
                    &imported,
                    &crate::state_store::ScopeShards {
                        sources: &sources,
                        ..Default::default()
                    },
                )
                .map(drop),
            store
                .begin_verification(
                    scope(),
                    crate::state_store::BeginVerificationInput {
                        rule: Some("rule_absent".to_string()),
                        declaration: None,
                        key: "check".to_string(),
                        method: "examples".to_string(),
                        declared_by: "tester".to_string(),
                        file: Some("tests/check.rs".into()),
                        symbol: None,
                        commit: None,
                    },
                    provenance_core::VerificationMethod::Examples,
                )
                .map(drop),
            store
                .complete_verification(
                    &scope(),
                    crate::state_store::CompleteVerificationInput {
                        run: "run_absent".to_string(),
                        status: "passed".to_string(),
                        error: None,
                    },
                )
                .map(drop),
        ];
        for refusal in refusals {
            assert!(refusal.unwrap_err().to_string().contains("cannot write"));
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(std::fs::read(&shard).unwrap(), before);
    for path in [
        copy.scopes_dir().join(imported.as_str()),
        copy.verification_runs_path(&scope()),
        copy.verification_runs_lock_path(&scope()),
        copy.publication_lock_path(),
        copy.cache_dir(),
        copy.import_transactions_dir(),
    ] {
        assert!(!path.exists(), "{path} exists");
    }
}
