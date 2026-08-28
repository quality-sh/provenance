use super::*;
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
            "schema_version": 1,
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
            "schema_version": 1,
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
            "schema_version": 1,
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
            "schema_version": 1,
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
            "schema_version": 1,
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
            "schema_version": 1,
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
            "schema_version": 1,
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
    let key = layout.publication_lock_path().to_string();
    assert!(!guard::nested_lock_held(&key));

    let transaction = layout.import_transactions_dir().join("completed");
    std::fs::create_dir(&transaction).unwrap();
    write_publication_marker(&layout, &transaction, PublicationPhase::Published).unwrap();
    std::fs::remove_dir(transaction).unwrap();

    with_repository_publication(&layout, || Ok(())).unwrap();

    assert!(!layout.publication_marker_path().exists());
}

#[test]
fn state_snapshot_waits_for_complete_publication() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let (midpoint_tx, midpoint_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let publisher = {
        let layout = layout.clone();
        std::thread::spawn(move || {
            with_repository_publication(&layout, || {
                std::fs::write(layout.state_dir().join("first"), "new").unwrap();
                midpoint_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                std::fs::write(layout.state_dir().join("second"), "new").unwrap();
                Ok(())
            })
            .unwrap();
        })
    };
    midpoint_rx.recv().unwrap();

    let (snapshot_tx, snapshot_rx) = std::sync::mpsc::channel();
    let snapshotter =
        std::thread::spawn(move || snapshot_tx.send(snapshot_state(&layout).unwrap()).unwrap());
    assert!(snapshot_rx
        .recv_timeout(std::time::Duration::from_millis(100))
        .is_err());
    release_tx.send(()).unwrap();
    publisher.join().unwrap();
    let snapshot = snapshot_rx.recv().unwrap();
    snapshotter.join().unwrap();

    assert_eq!(
        std::fs::read_to_string(snapshot.layout().state_dir().join("first")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_to_string(snapshot.layout().state_dir().join("second")).unwrap(),
        "new"
    );
}

#[cfg(unix)]
#[test]
fn state_snapshot_rejects_external_file_symlink() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let external = directory.path().join("secret");
    std::fs::write(&external, "do not snapshot").unwrap();
    std::os::unix::fs::symlink(&external, layout.state_dir().join("external-link")).unwrap();

    let error = match snapshot_state(&layout) {
        Ok(_) => panic!("snapshot unexpectedly followed an external symlink"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("unsupported state entry"), "{error}");
    assert_eq!(
        std::fs::read_to_string(external).unwrap(),
        "do not snapshot"
    );
}
