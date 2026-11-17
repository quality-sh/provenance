//! The check of the scope of a scoped call in the gap of a publication.
use super::super::{invoke_authorized_typed, ListThreads, RequestedContext};
use crate::{layout::ProvenanceLayout, operations::local_repository::tests::repository};
use provenance_core::protocol::repository::RepositoryScope;
use provenance_macros::verifies;
use std::sync::Arc;

/// The probe runs a scoped read through the binding while the publication
/// holds its lock with its files installed. The check of the scope reads
/// the manifest, so the read must wait for the publication and not fail.
#[test]
#[verifies("rule_preparation_reads_published_state", examples)]
fn a_scoped_read_in_the_publication_gap_succeeds_after_the_publication() {
    let (dir, binding) = repository();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    let reader = std::rc::Rc::new(std::cell::RefCell::new(None));
    crate::test_probes::arm("state_installed", {
        let reader = reader.clone();
        let mut binding = Some(binding);
        move || {
            let binding = binding.take().expect("one publication");
            let (done, finished) = std::sync::mpsc::channel();
            reader.replace(Some(std::thread::spawn(move || {
                let context = RequestedContext::Scope(RepositoryScope {
                    repository: "native".into(),
                    scope: "default".into(),
                });
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let threads = runtime
                    .block_on(invoke_authorized_typed::<ListThreads>(
                        Arc::new(binding),
                        context,
                        (),
                    ))
                    .map_err(|error| format!("{error:?}"));
                let _ = done.send(());
                threads
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
            // Two new shards, so the stage lands as a whole set.
            |staged| {
                let scope = provenance_core::ScopeId::new("default")?;
                staged.publish_shards(
                    [
                        provenance_core::NodeType::Topic,
                        provenance_core::NodeType::Question,
                    ]
                    .map(|kind| (crate::shards::path_for(&layout, &scope, kind), Vec::new()))
                    .into(),
                )
            },
        )
    })
    .unwrap();
    crate::test_probes::disarm("state_installed");
    let reader = reader.take().expect("the publication reached the gap");
    assert_eq!(reader.join().unwrap(), Ok(Vec::new()));
}
