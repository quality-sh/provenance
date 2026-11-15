//! The grant check of a scoped call in the gap of a publication.
use super::super::{ContextResolver, PreparedContext, RequestedContext};
use crate::{layout::ProvenanceLayout, operations::LocalRepository, repository::Repository};
use provenance_core::{protocol::repository::RepositoryContext, Manifest, RepoPathPrefix, ScopeId};
use provenance_macros::verifies;

fn repository() -> (tempfile::TempDir, LocalRepository) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest =
        Manifest::default_with_scope(ScopeId::new("default").unwrap(), RepoPathPrefix::new("."));
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let opened = Repository::open(dir.path(), "default").unwrap();
    let binding = LocalRepository::bind(opened, "native").unwrap();
    (dir, binding)
}

/// The probe prepares a host call and takes its scope while the publication
/// holds its lock with the live state moved away. The grant check reads the
/// manifest, so the call must wait for the publication and not fail.
#[test]
#[verifies("rule_preparation_reads_published_state", examples)]
fn the_grant_check_in_the_publication_gap_succeeds_after_the_publication() {
    let (dir, binding) = repository();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    let reader = std::rc::Rc::new(std::cell::RefCell::new(None));
    crate::test_probes::arm("state_after_backup_rename", {
        let reader = reader.clone();
        let mut binding = Some(binding);
        move || {
            let binding = binding.take().expect("one publication");
            let (done, finished) = std::sync::mpsc::channel();
            reader.replace(Some(std::thread::spawn(move || {
                let context = RequestedContext::Scoped(RepositoryContext {
                    repository: "native".into(),
                    scope: "default".into(),
                    freshness: None,
                });
                let scope = binding
                    .prepare("search", context, &[])
                    .map_err(anyhow::Error::from)
                    .and_then(PreparedContext::scope)
                    .map(|_| ())
                    .map_err(|error| format!("{error:#}"));
                let _ = done.send(());
                scope
            })));
            // A call that does not wait ends in the gap. A call that waits ends only
            // after the publication, so this wait times out.
            let _ = finished.recv_timeout(std::time::Duration::from_secs(1));
            Ok(())
        }
    });
    crate::publication::with_repository_publication(&layout, |held| {
        crate::publication::with_staged_state(
            held,
            false,
            crate::state_store::RecordStamp::ResolveFromRepository,
            |_| Ok(()),
        )
    })
    .unwrap();
    crate::test_probes::disarm("state_after_backup_rename");
    let reader = reader.take().expect("the publication reached the gap");
    assert_eq!(reader.join().unwrap(), Ok(()));
}
