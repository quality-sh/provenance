//! The contract case of an entry that is not a file at the path of a file,
//! on each adapter. The memory adapter tests are implementation aids with no
//! Rule binding: they show that the memory adapter gives the answers of Git.

use super::git_harness::git_harness;
use super::wrong::{self, Wrong};
use super::Memory;
use provenance_macros::verifies;
use provenance_store::review::history::contract::{self, Entry, UNKNOWN_COMMIT};

/// A submodule entry to a commit that the repository does not hold.
fn absent_submodule() -> Entry {
    Entry::submodule(UNKNOWN_COMMIT)
}

/// A submodule entry to `base`, a commit that the repository holds.
fn held_submodule() -> Entry {
    Entry::submodule("base")
}

fn directory() -> Entry {
    Entry::directory()
}

#[tokio::test]
#[verifies("rule_record_history_text_only_from_file", examples)]
async fn git_a_submodule_entry_to_an_absent_commit_has_no_text() {
    contract::an_entry_that_is_not_a_file_has_no_text(&git_harness(), absent_submodule()).await;
}

#[tokio::test]
#[verifies("rule_record_history_text_only_from_file", examples)]
async fn git_a_submodule_entry_to_a_held_commit_has_no_text() {
    contract::an_entry_that_is_not_a_file_has_no_text(&git_harness(), held_submodule()).await;
}

#[tokio::test]
#[verifies("rule_record_history_text_only_from_file", examples)]
async fn git_a_directory_at_the_path_of_a_file_has_no_text() {
    contract::an_entry_that_is_not_a_file_has_no_text(&git_harness(), directory()).await;
}

#[tokio::test]
async fn memory_a_submodule_entry_to_an_absent_commit_has_no_text() {
    contract::an_entry_that_is_not_a_file_has_no_text(&Memory, absent_submodule()).await;
}

#[tokio::test]
async fn memory_a_submodule_entry_to_a_held_commit_has_no_text() {
    contract::an_entry_that_is_not_a_file_has_no_text(&Memory, held_submodule()).await;
}

#[tokio::test]
async fn memory_a_directory_at_the_path_of_a_file_has_no_text() {
    contract::an_entry_that_is_not_a_file_has_no_text(&Memory, directory()).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/not_a_file.jsonl: the listed commits, parents or texts are not the expected ones"
)]
async fn the_contract_refuses_an_adapter_that_gives_a_text_for_a_submodule_entry() {
    let wrong = Wrong(wrong::entry_text);
    contract::an_entry_that_is_not_a_file_has_no_text(&wrong, held_submodule()).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/not_a_file.jsonl: the listed commits, parents or texts are not the expected ones"
)]
async fn the_contract_refuses_an_adapter_that_gives_a_text_for_a_directory() {
    let wrong = Wrong(wrong::entry_text);
    contract::an_entry_that_is_not_a_file_has_no_text(&wrong, directory()).await;
}
