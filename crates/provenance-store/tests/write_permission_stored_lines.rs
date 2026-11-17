//! A Requirement save over a proposal card line that the line reader refuses.
//!
//! The write permission check reads the proposal cards of the scope to find
//! the records with a submitted review, before any other read of that file
//! in the save. It reads each line through the line reader of the file, so a
//! line that holds a top-level member two times, or that is not one JSON
//! value, stops the save with `invalid_state_file` at the line, and the save
//! writes nothing. These tests have no Rule binding: no active Rule states
//! the refusal of a stored line that holds a member two times.

use crate::review_support::*;
use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_store::write_error::{WriteError, WriteFailure};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Each file below `directory` with its bytes.
fn files(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(self::files(&path));
        } else {
            files.insert(path.clone(), std::fs::read(&path).unwrap());
        }
    }
    files
}

/// The file, line, message and hint of the `invalid_state_file` refusal of
/// a Requirement save in a fixture whose proposal card file holds `cards`.
/// The save changes no file of the state.
fn refusal_of_save(cards: &str) -> (String, u64, String, Option<String>) {
    let (temp, store) = fixture();
    let request = save(&store, json!({"description":"Described."}));
    let layout = provenance_store::layout::ProvenanceLayout::new(
        camino::Utf8Path::from_path(temp.path()).unwrap(),
    );
    let path = provenance_store::shards::proposal_cards_path(&layout, &scope());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, cards).unwrap();
    let state = temp.path().join(".provenance/state");
    let before = files(&state);

    let error = store.save_requirement(request).unwrap_err();

    assert_eq!(files(&state), before);
    match WriteError::from(error).safe() {
        WriteFailure::InvalidStateFile {
            file,
            line,
            message,
            hint,
        } => (file, line, message, hint),
        other => panic!("the save gave {other:?}"),
    }
}

#[test]
fn a_save_refuses_a_proposal_card_line_that_holds_a_member_two_times() {
    let refusal =
        refusal_of_save("{\"proposal_type\":\"note\",\"proposal_type\":\"record_revision\"}\n");

    assert_eq!(
        refusal,
        (
            "scopes/default/ideation/proposal_cards.jsonl".to_owned(),
            1,
            "duplicate field `proposal_type`".to_owned(),
            Some(STATE_CORRECTIONS.to_owned()),
        )
    );
}

#[test]
fn a_save_refuses_a_proposal_card_line_that_is_not_json() {
    let refusal = refusal_of_save("<<<<<<< HEAD\n");

    assert_eq!(
        refusal,
        (
            "scopes/default/ideation/proposal_cards.jsonl".to_owned(),
            1,
            "expected value".to_owned(),
            Some(STATE_CORRECTIONS.to_owned()),
        )
    );
}
