//! The history operation over a folder repository with real Git history,
//! when the author name of one commit is larger than the page bytes: the
//! same pages as over the memory adapter. Git writes each commit object
//! from its text, so the author name is in no command argument and in no
//! environment string.

use super::history::fixture;
use crate::history_folder::folder;
use crate::history_long::{long_history, long_name};
use crate::history_pages::follow;
use provenance_macros::verifies;
use provenance_store::operations::read_operations::InProcessReadOperations;
use provenance_store::operations::LocalRepository;
use provenance_store::repository::Repository;
use serde_json::{json, Value};

/// The ids of the entries of each page of `review-history` for `req_a`,
/// over a folder of three commits that change `req_a`, where the author
/// name of `edit_001` has `author` bytes. The read follows `next_cursor`
/// from the first page. A failed page gives the pages before it and its
/// error.
async fn pages(author: usize) -> (Vec<Vec<String>>, Option<Value>) {
    let mut history = fixture();
    let mut commits = long_history(&history.working);
    commits.truncate(3);
    commits[1].author = "a".repeat(author);
    history.commits = commits;
    let folder = folder(&history);
    let repository = Repository::open(folder.root(), "default").unwrap();
    let binding = LocalRepository::bind(repository, "folder").unwrap();
    let operations = InProcessReadOperations::new(&binding);
    follow(&operations, |id| folder.named(id)).await
}

/// A commit larger than the page bytes and smaller than the response
/// budget is alone on its page.
///
/// Implementation aid, no Rule: no Rule in the graph says a page with a cursor holds an entry.
#[tokio::test]
async fn a_commit_larger_than_a_page_is_alone_on_its_page() {
    assert_eq!(
        pages(1_050_000).await,
        (
            vec![
                vec![long_name(0)],
                vec![long_name(1)],
                vec![long_name(2), "working".to_owned()],
            ],
            None
        )
    );
}

/// A commit larger than the response budget gives the explicit budget
/// refusal on page 2, its page, and page 1 gives the commit before it.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_commit_larger_than_the_budget_gives_page_budget_exceeded() {
    assert_eq!(
        pages(1_200_000).await,
        (
            vec![vec![long_name(0)]],
            Some(json!({"kind": "page_budget_exceeded"}))
        )
    );
}
