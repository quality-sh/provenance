//! The edge cases of the file versions contract on the git adapter and on
//! the memory adapter, and the wrong adapter that each case refuses.

use super::git_harness::git_harness;
use super::wrong::{self, Wrong};
use super::Memory;
use provenance_store::review::history::contract;

#[tokio::test]
async fn git_an_octopus_merge_goes_on_through_the_parent_with_its_file() {
    contract::an_octopus_merge_goes_on_through_the_parent_with_its_file(&git_harness()).await;
}

#[tokio::test]
async fn git_a_commit_on_two_parent_lines_is_listed_once() {
    contract::a_commit_on_two_parent_lines_is_listed_once(&git_harness()).await;
}

#[tokio::test]
async fn git_a_change_of_mode_only_is_listed() {
    contract::a_change_of_mode_only_is_listed(&git_harness()).await;
}

#[tokio::test]
async fn git_a_commit_that_the_adapter_does_not_hold_has_no_text() {
    contract::a_commit_that_the_adapter_does_not_hold_has_no_text(&git_harness()).await;
}

#[tokio::test]
async fn git_a_text_that_is_not_utf8_is_an_error() {
    contract::a_text_that_is_not_utf8_is_an_error(&git_harness()).await;
}

#[tokio::test]
async fn memory_an_octopus_merge_goes_on_through_the_parent_with_its_file() {
    contract::an_octopus_merge_goes_on_through_the_parent_with_its_file(&Memory).await;
}

#[tokio::test]
async fn memory_a_commit_on_two_parent_lines_is_listed_once() {
    contract::a_commit_on_two_parent_lines_is_listed_once(&Memory).await;
}

#[tokio::test]
async fn memory_a_change_of_mode_only_is_listed() {
    contract::a_change_of_mode_only_is_listed(&Memory).await;
}

#[tokio::test]
async fn memory_a_commit_that_the_adapter_does_not_hold_has_no_text() {
    contract::a_commit_that_the_adapter_does_not_hold_has_no_text(&Memory).await;
}

#[tokio::test]
async fn memory_a_text_that_is_not_utf8_is_an_error() {
    contract::a_text_that_is_not_utf8_is_an_error(&Memory).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/octopus.jsonl: the listed commits, parents or texts are not the expected ones"
)]
async fn the_contract_refuses_an_adapter_that_compares_an_octopus_merge_with_two_parents_only() {
    contract::an_octopus_merge_goes_on_through_the_parent_with_its_file(&Wrong(
        wrong::first_two_parents,
    ))
    .await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/two_lines.jsonl: the commit base is listed more than once"
)]
async fn the_contract_refuses_an_adapter_that_lists_a_commit_once_per_parent_line() {
    contract::a_commit_on_two_parent_lines_is_listed_once(&Wrong(wrong::each_line)).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/mode.jsonl: the listed commits, parents or texts are not the expected ones"
)]
async fn the_contract_refuses_an_adapter_that_does_not_list_a_change_of_mode() {
    contract::a_change_of_mode_only_is_listed(&Wrong(wrong::ignores_mode)).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/octopus.jsonl: the text at a commit that the adapter does not hold is not None"
)]
async fn the_contract_refuses_an_adapter_that_gives_a_text_at_a_commit_it_does_not_hold() {
    contract::a_commit_that_the_adapter_does_not_hold_has_no_text(&Wrong(wrong::head_text)).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/unreadable.jsonl: a text that is not UTF-8 is not an error"
)]
async fn the_contract_refuses_an_adapter_that_replaces_bytes_that_are_not_utf8() {
    contract::a_text_that_is_not_utf8_is_an_error(&Wrong(wrong::lossy_text)).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/unreadable.jsonl: the error is not the UTF-8 error of the text at base"
)]
async fn the_contract_refuses_an_adapter_whose_text_fails_for_another_reason() {
    contract::a_text_that_is_not_utf8_is_an_error(&Wrong(wrong::failed_text)).await;
}
