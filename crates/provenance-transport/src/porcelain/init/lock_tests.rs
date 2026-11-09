//! Init and readers of the state: a read never sees the manifest moved away.
use super::{prepare_init, InitOptions};
use camino::Utf8PathBuf;
use provenance_store::layout::ProvenanceLayout;
use std::sync::mpsc;
use std::time::Duration;

fn options(actor: &str) -> InitOptions {
    InitOptions {
        scope: Some("default".to_owned()),
        path_prefix: Some(Utf8PathBuf::from(".")),
        disposition_actor_ids: vec![actor.to_owned()],
        clear_disposition_actors: false,
        ste_pdf: None,
        invocation_channel: provenance_porcelain::init::InvocationChannel::Native,
        package_manager: None,
        skills: crate::skills::TEST_SKILLS,
    }
}

/// A reader holds the publication lock. A second init must not change the
/// manifest until the reader releases the lock; then it lands.
#[test]
#[provenance_macros::verifies("rule_init_reads_get_whole_state", examples)]
fn second_init_waits_for_the_publication_lock() {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    prepare_init(&repo, options("reviewer"))
        .unwrap()
        .apply()
        .unwrap();
    let layout = ProvenanceLayout::new(repo.clone());
    let before = std::fs::read(layout.manifest_path()).unwrap();
    let plan = prepare_init(&repo, options("new-reviewer")).unwrap();

    let (done_sender, done) = mpsc::channel();
    let held = provenance_store::publication::with_repository_publication(&layout, |_| {
        let writer = std::thread::spawn(move || {
            let result = plan.apply().map(|_| ());
            let _ = done_sender.send(());
            result
        });
        // An init that ignores the lock ends in milliseconds. A correct init still
        // waits when this bound ends.
        let finished = done.recv_timeout(Duration::from_secs(2)).is_ok();
        let during = std::fs::read(layout.manifest_path())?;
        Ok((writer, finished, during))
    });
    let (writer, finished, during) = held.unwrap();
    assert!(!finished, "init finished while a reader held the lock");
    assert_eq!(during, before, "init changed the manifest under a reader");
    writer.join().unwrap().unwrap();
    let after = String::from_utf8(std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    assert!(after.contains("new-reviewer"), "{after}");
}
