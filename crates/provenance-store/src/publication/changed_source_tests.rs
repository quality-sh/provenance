//! A source edit whose state install stops or fails after one of its two
//! state files, and a person who then changes the Source file. The section
//! entry completes the state before it refuses for the changed file.

use super::source_edit_recovery_tests::{canonical_root, AFTER, BEFORE};
use crate::stopped_child::{run_as_child, stop_in_child};
use crate::{
    layout::ProvenanceLayout,
    operations::files::RepositoryFiles,
    publication::{
        with_repository_publication, with_staged_state_and_source_edit, SourceEditRecoveryFailure,
    },
    test_probes,
};
use camino::Utf8Path;
use provenance_macros::verifies;

const KEYS: [&str; 2] = [
    "scopes/a/questions/question.jsonl",
    "scopes/a/rules/rule.jsonl",
];

/// A repository with the Source file and the two state files at `BEFORE`.
fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = canonical_root(&temp);
    let layout = ProvenanceLayout::new(&root);
    for key in KEYS {
        let path = layout.state_dir().join(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, BEFORE).unwrap();
    }
    std::fs::write(root.join("source.txt"), BEFORE).unwrap();
    temp
}

/// A source edit of `source.txt` that puts `AFTER` in each state file.
fn publish(root: &Utf8Path) -> anyhow::Result<()> {
    let layout = ProvenanceLayout::new(root);
    let held = RepositoryFiles::open(root)
        .unwrap()
        .read_bounded(Utf8Path::new("source.txt"), 1024)
        .unwrap();
    with_repository_publication(&layout, |section| {
        with_staged_state_and_source_edit(section, held, AFTER, |staged| {
            for key in KEYS {
                staged.publish_shard(&staged.layout.state_dir().join(key), AFTER.to_vec())?;
            }
            Ok(())
        })
    })
}

const CHILD: &str = "operations::files::tests::changed_source_tests::two_key_child";

#[test]
fn two_key_child() {
    run_as_child(&["state_file_installed"], |root| publish(root).unwrap());
}

fn states(layout: &ProvenanceLayout) -> Vec<Vec<u8>> {
    KEYS.iter()
        .map(|key| std::fs::read(layout.state_dir().join(key)).unwrap())
        .collect()
}

/// A person writes the Source file, and then a section entry runs. The
/// entry refuses for the changed file, the state is the state after the
/// edit, the person's file stays, and the marker stays.
fn assert_refused_with_a_whole_state(root: &Utf8Path) {
    std::fs::write(root.join("source.txt"), b"person\n").unwrap();
    let layout = ProvenanceLayout::new(root);
    for entry in 0..2 {
        let error = with_repository_publication(&layout, |_| Ok(())).unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<SourceEditRecoveryFailure>(),
                Some(SourceEditRecoveryFailure::ExternalChange { .. })
            ),
            "{entry}: {error:#}"
        );
        assert_eq!(states(&layout), [AFTER, AFTER], "{entry}");
        assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), b"person\n");
        assert!(layout.source_edit_marker_path().exists(), "{entry}");
    }
}

/// The process stops after the first state file is installed. The Source
/// file is new, and one state file is new and one is old.
#[test]
#[verifies("rule_changed_source_refusal_keeps_a_whole_state", examples)]
fn a_stop_in_the_state_install_and_a_person_change_give_the_whole_state_and_the_refusal() {
    let temp = fixture();
    let root = canonical_root(&temp);
    stop_in_child(CHILD, &root, "state_file_installed");
    let layout = ProvenanceLayout::new(&root);
    assert_eq!(states(&layout), [AFTER, BEFORE]);

    assert_refused_with_a_whole_state(&root);
}

/// The state install fails after the first state file, as a held file
/// makes it fail on Windows. The write succeeds with its marker.
#[test]
#[verifies("rule_changed_source_refusal_keeps_a_whole_state", examples)]
fn a_failed_state_install_and_a_person_change_give_the_whole_state_and_the_refusal() {
    let temp = fixture();
    let root = canonical_root(&temp);
    test_probes::arm("state_file_installed", || anyhow::bail!("injected failure"));
    let written = publish(&root);
    test_probes::disarm("state_file_installed");
    written.unwrap();
    let layout = ProvenanceLayout::new(&root);
    assert_eq!(states(&layout), [AFTER, BEFORE]);
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), AFTER);

    assert_refused_with_a_whole_state(&root);
}
