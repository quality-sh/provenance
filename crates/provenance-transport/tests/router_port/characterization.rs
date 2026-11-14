//! Characterization: the answers of the operations that the review page
//! calls, apart from record history, through the HTTP routes of a host over
//! a repository in a folder. The checked transcript
//! `tests/fixtures/site_operations.json` pins them apart from the generated
//! ids, digests and times, so a change to how the router runs these
//! operations cannot change an answer or its order. A failure shows the
//! lines that differ.

use crate::support::records::{allow_reviewer, Repository};
use crate::support::router_answer::Http;
use crate::support::site_operations::{assert_pinned, run};

/// Implementation aid: characterization of the answers of the router when it
/// runs each operation itself. No Rule binds it.
#[tokio::test]
async fn the_site_operations_answer_as_the_checked_transcript() {
    let _alone = crate::one_at_a_time().await;
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    allow_reviewer(&repo);
    let root = repo.dir.path().to_str().unwrap().to_owned();
    let transcript = run(&Http::over(repo.open()), &root).await;
    assert_eq!(transcript.answers.len(), 67);
    assert_pinned(&transcript.pinned());
}
