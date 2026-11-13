//! Characterization: the answers of the operations that the review page
//! calls, apart from record history, through the HTTP routes of a host over
//! a repository in a folder. The transcript is pinned apart from the
//! generated ids, digests and times, so a change to how the router runs
//! these operations cannot change an answer or its order.
#![cfg(feature = "test-fixture")]

// Each test file uses a part of the shared support.
#[allow(dead_code)]
mod support {
    pub mod catch_up_steps;
    pub mod records;
    pub mod resource_http;
    pub mod site_operations;
    pub mod transcript;
}

use support::records::{allow_reviewer, Repository};
use support::site_operations::{run, Http};
use support::transcript::fingerprint;

const SITE_OPERATIONS_FINGERPRINT: &str = "d0cca63d4265d0bb";

/// Implementation aid: characterization of the router's answers, made
/// before the router can send read operations to a port. No Rule binds it.
#[tokio::test]
async fn the_site_operations_answer_as_before() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    allow_reviewer(&repo);
    let root = repo.dir.path().to_str().unwrap().to_owned();
    let transcript = run(&Http::over(repo.open()), &root).await.named();
    let calls = transcript.len();
    assert_eq!(calls, 67, "{transcript:#?}");
    assert_eq!(
        fingerprint(&transcript),
        SITE_OPERATIONS_FINGERPRINT,
        "{}",
        serde_json::to_string_pretty(&transcript).unwrap()
    );
}
