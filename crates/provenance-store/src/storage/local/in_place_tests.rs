//! The install in place of a whole set and its completion at the next
//! section entry, seen through the files that a stopped write leaves.

use crate::layout::ProvenanceLayout;
use crate::publication::{with_repository_publication, write_publication_marker, PublicationPhase};
use crate::test_support::directory::utf8;
use camino::Utf8PathBuf;
use provenance_macros::verifies;

const RULES: &str = "scopes/default/rules/rule.jsonl";
const REQUIREMENTS: &str = "scopes/default/requirements/req.jsonl";
const QUESTIONS: &str = "scopes/default/questions/question.jsonl";

/// A layout with two live shards, `RULES` and `QUESTIONS`, at `old`.
fn live() -> (tempfile::TempDir, ProvenanceLayout) {
    let temp = tempfile::tempdir().unwrap();
    // The canonical root, so that a path in an error has the spelling
    // that the store gives on macOS, where the temporary directory is
    // below a link.
    let layout = ProvenanceLayout::new(utf8(&dunce::canonicalize(temp.path()).unwrap()));
    for key in [RULES, QUESTIONS] {
        let path = layout.state_dir().join(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "old").unwrap();
    }
    std::fs::create_dir_all(layout.import_transactions_dir()).unwrap();
    (temp, layout)
}

/// A transaction as a write leaves it when it stops after its marker: the
/// staged puts that `staged` names, the change list, and the marker.
fn stopped_write(
    layout: &ProvenanceLayout,
    puts: &[&str],
    staged: &[&str],
    deletes: &[&str],
) -> Utf8PathBuf {
    let transaction = layout.import_transactions_dir().join("stopped");
    for key in staged {
        let path = transaction.join("staged").join(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "new").unwrap();
    }
    std::fs::create_dir_all(&transaction).unwrap();
    std::fs::write(
        transaction.join("changes.json"),
        serde_json::json!({ "puts": puts, "deletes": deletes }).to_string(),
    )
    .unwrap();
    write_publication_marker(layout, &transaction, PublicationPhase::Installing).unwrap();
    transaction
}

fn read(layout: &ProvenanceLayout, key: &str) -> Option<String> {
    std::fs::read_to_string(layout.state_dir().join(key)).ok()
}

fn enter(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    with_repository_publication(layout, |_| Ok(()))
}

#[test]
#[verifies("rule_stopped_write_completes_after_its_marker", examples)]
fn the_next_section_entry_installs_each_put_and_removes_each_delete() {
    let (_temp, layout) = live();
    let transaction = stopped_write(
        &layout,
        &[RULES, REQUIREMENTS],
        &[RULES, REQUIREMENTS],
        &[QUESTIONS],
    );

    enter(&layout).unwrap();

    assert_eq!(read(&layout, RULES).as_deref(), Some("new"));
    assert_eq!(read(&layout, REQUIREMENTS).as_deref(), Some("new"));
    assert_eq!(read(&layout, QUESTIONS), None);
    assert!(!transaction.exists());
    assert!(!layout.publication_marker_path().exists());
}

/// The write stopped after it installed `RULES`: the staged file of that
/// put is gone, and the live file has its new bytes.
#[test]
#[verifies("rule_stopped_write_completes_after_its_marker", examples)]
fn the_next_section_entry_completes_an_install_that_stopped_in_part() {
    let (_temp, layout) = live();
    std::fs::write(layout.state_dir().join(RULES), "new").unwrap();
    stopped_write(&layout, &[RULES, REQUIREMENTS], &[REQUIREMENTS], &[]);

    enter(&layout).unwrap();

    assert_eq!(read(&layout, RULES).as_deref(), Some("new"));
    assert_eq!(read(&layout, REQUIREMENTS).as_deref(), Some("new"));
    assert_eq!(read(&layout, QUESTIONS).as_deref(), Some("old"));
    assert!(!layout.publication_marker_path().exists());
}

/// A change list that names a path outside the state directory is not one
/// that a write makes, so the test writes it itself. This test is an
/// implementation aid with no Rule.
#[test]
fn the_install_refuses_a_change_list_with_a_path_outside_the_state() {
    for key in [
        "../outside.jsonl",
        "scopes/default/../../../outside.jsonl",
        "C:outside.jsonl",
    ] {
        let (_temp, layout) = live();
        let outside = layout.provenance_dir().join("outside.jsonl");
        std::fs::write(&outside, "keep").unwrap();
        stopped_write(&layout, &[key], &[], &[key]);

        let error = enter(&layout).unwrap_err();

        let path = layout
            .import_transactions_dir()
            .join("stopped")
            .join("changes.json");
        assert_eq!(
            format!("{error:#}"),
            format!("change list {path} names {key}, which is not a state key"),
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep", "{key}");
        assert!(layout.publication_marker_path().exists(), "{key}");
    }
}

/// A marker that an earlier release wrote names a phase that this store
/// does not have. The section entry refuses it with the path of the marker
/// and changes no file. This test is an implementation aid with no Rule.
#[test]
fn the_section_entry_refuses_a_marker_of_an_earlier_format() {
    for phase in ["prepared", "backup_created"] {
        let (_temp, layout) = live();
        let transaction = layout.import_transactions_dir().join("earlier");
        std::fs::create_dir_all(&transaction).unwrap();
        let marker = layout.publication_marker_path();
        std::fs::write(
            &marker,
            serde_json::json!({ "transaction_dir": transaction, "phase": phase }).to_string(),
        )
        .unwrap();

        let error = format!("{:#}", enter(&layout).unwrap_err());

        let repair = "the marker is not in the format of this release: open the repository \
                      one time with the release that wrote the marker, so that its write \
                      completes";
        assert!(
            error.starts_with(&format!(
                "read {marker} ({repair}): unknown variant `{phase}`"
            )),
            "{error}"
        );
        assert_eq!(read(&layout, RULES).as_deref(), Some("old"), "{phase}");
        assert!(transaction.is_dir(), "{phase}");
        assert!(marker.is_file(), "{phase}");
    }
}

/// A reader keeps the live file open with the sharing of `std::fs`, which
/// lets other handles delete the file. The install moves the staged file
/// from the cache onto it.
#[test]
#[verifies("rule_write_passes_while_a_record_file_is_held", examples)]
fn the_install_replaces_a_live_file_that_a_reader_holds() {
    let (_temp, layout) = live();
    stopped_write(&layout, &[RULES], &[RULES], &[]);
    let reader = std::fs::File::open(layout.state_dir().join(RULES)).unwrap();

    let entered = enter(&layout);
    drop(reader);

    entered.unwrap();
    assert_eq!(read(&layout, RULES).as_deref(), Some("new"));
    assert!(!layout.publication_marker_path().exists());
}

/// A reader keeps the live file open with a share mode that lets other
/// handles only read. The roll forward cannot replace the file, so the
/// section entry answers `state_file_in_use` with the file, and the marker
/// stays. When the reader closes the file, the next entry completes the
/// write.
#[cfg(windows)]
#[test]
#[verifies("rule_held_record_file_refusal_names_the_file", examples)]
fn a_file_that_a_reader_holds_with_no_delete_sharing_is_refused_by_name() {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    let (_temp, layout) = live();
    stopped_write(&layout, &[RULES], &[RULES], &[]);
    let reader = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(layout.state_dir().join(RULES))
        .unwrap();

    let entered = enter(&layout);
    drop(reader);

    let error = crate::write_error::WriteError(entered.unwrap_err());
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        serde_json::json!({"kind": "state_file_in_use", "file": format!(".provenance/state/{RULES}")})
    );
    assert_eq!(error.status(), 503);
    assert!(layout.publication_marker_path().exists());
    enter(&layout).unwrap();
    assert_eq!(read(&layout, RULES).as_deref(), Some("new"));
}
