use super::{seeded_source_requirement_store, staged_store};
use crate::publication::with_repository_publication;
use crate::shards;
use crate::state_store::{ScopeShards, StateStore};
use provenance_core::ScopeId;
use provenance_macros::verifies;
use std::sync::mpsc;
use std::time::Duration;

fn assert_import_refused(store: &StateStore, scope: &ScopeId) {
    let error = store
        .import_scope(scope, &ScopeShards::default())
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "a scope import needs a staged store of {}",
            store.layout.root()
        )
    );
}

#[test]
#[verifies("rule_scope_import_needs_a_staged_store", examples)]
fn a_scope_import_on_a_live_store_is_refused_and_changes_nothing() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let discussions = shards::discussions_path(&store.layout, &scope);
    std::fs::create_dir_all(discussions.parent().unwrap()).unwrap();
    std::fs::write(&discussions, "{\"id\":\"discussion_kept\"}\n").unwrap();
    let sources = shards::sources_path(&store.layout, &scope);
    let before = [
        std::fs::read(&sources).unwrap(),
        std::fs::read(&discussions).unwrap(),
    ];

    assert_import_refused(&store, &scope);
    with_repository_publication(&store.layout, |section| {
        assert_import_refused(section, &scope);
        Ok(())
    })
    .unwrap();

    assert_eq!(
        [
            std::fs::read(&sources).unwrap(),
            std::fs::read(&discussions).unwrap(),
        ],
        before
    );
}

#[test]
#[verifies("rule_scope_import_needs_a_staged_store", examples)]
#[verifies("rule_staged_sections_run_one_at_a_time", examples)]
fn a_staged_import_holds_the_stage_lock() {
    let (_dir, _live, staged, scope, _) = staged_store();
    let (started, on_start) = mpsc::channel();
    let (release, on_release) = mpsc::channel::<()>();
    let (entered, on_entry) = mpsc::channel();
    let (staged, scope) = (&staged, &scope);
    std::thread::scope(|threads| {
        let import = threads.spawn(move || {
            crate::test_probes::arm("scope_import_sources_written", move || {
                started.send(()).unwrap();
                on_release.recv().unwrap();
                Ok(())
            });
            let result = staged.import_scope(scope, &ScopeShards::default());
            crate::test_probes::disarm("scope_import_sources_written");
            result
        });
        on_start.recv_timeout(Duration::from_secs(5)).unwrap();
        threads.spawn(move || {
            staged.with_repository_publication(|_| {
                entered.send(()).unwrap();
                Ok(())
            })
        });
        let waited = on_entry.recv_timeout(Duration::from_millis(300));
        release.send(()).unwrap();
        assert_eq!(
            waited,
            Err(mpsc::RecvTimeoutError::Timeout),
            "the section waits for the import"
        );
        import.join().unwrap().unwrap();
        assert_eq!(on_entry.recv_timeout(Duration::from_secs(5)), Ok(()));
    });
}

#[test]
fn a_staged_section_store_imports_with_no_second_lock() {
    let (_dir, _live, staged, scope, _) = staged_store();
    staged
        .with_repository_publication(|section| {
            section.import_scope(&scope, &ScopeShards::default())
        })
        .unwrap();
    assert!(shards::sources_path(&staged.layout, &scope).exists());
}
