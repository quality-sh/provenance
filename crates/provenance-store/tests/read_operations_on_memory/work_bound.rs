//! The answer of a page that passes its work bound, with and without
//! `host`.
//!
//! The read database stops each statement that a transaction runs after
//! it sets its work bound, with `OverBudget`, as a store does for a
//! statement past the bound. Each page sets the bound before its first
//! statement, so each page passes its bound. The answer is the typed page
//! budget refusal, `page_budget_exceeded` with status 409, as the
//! operations declare it.

use crate::{context, handles_over, read};
use provenance_store::cache::catch_up_in_publish;
use provenance_store::operations::catalog::{
    frame_failure, invoke_authorized_typed, ContextKind, ContextResolver, Operation,
    RequestedContext,
};
use provenance_store::operations::family_operations::{
    PageAssertions, PageContributions, PageDiscussionContainers, PageDispositions, PageMessages,
    PageProposalAssertions, PageProposalDispositions, PageProposals, PageQuestions,
    PageRequirements, PageResolutions, PageRules, PageSources, PageSynthesisPackets, PageTopics,
    PageVerificationBindings,
};
use provenance_store::operations::read_operations::READ_OPERATIONS;
use provenance_store::operations::LocalRepository;
use provenance_store::read_database::contract::{Hooks, MakeHooks, Other};
use provenance_store::read_database::{
    OverBudget, ReadDatabase, SqlFuture, SqlRow, SqlValue, Transaction,
};
use provenance_store::repository::Repository;
use serde_json::{json, Value};
use std::sync::Arc;

/// Makes the [`PastBound`] hooks of each transaction.
#[derive(Debug)]
struct StopsPastBound;

impl MakeHooks for StopsPastBound {
    type Hooks = PastBound;
    fn make(&self) -> PastBound {
        PastBound(false)
    }
}

/// Whether the transaction set its work bound.
struct PastBound(bool);

impl Hooks for PastBound {
    fn fetch<'a>(
        &'a mut self,
        inner: &'a mut Transaction,
        sql: &'a str,
        values: Vec<SqlValue>,
    ) -> SqlFuture<'a, Vec<SqlRow>> {
        if self.0 {
            return Box::pin(async {
                Err(OverBudget(anyhow::Error::msg("the statement passed the work bound")).into())
            });
        }
        inner.fetch(sql, values)
    }

    fn bound_work<'a>(&'a mut self, inner: &'a mut Transaction, bound: u64) -> SqlFuture<'a, ()> {
        self.0 = true;
        inner.bound_work(bound)
    }
}

/// A repository whose read database stops each statement past the work
/// bound, after the embedder's catch-up.
async fn repository() -> (tempfile::TempDir, Repository) {
    let database: Arc<dyn ReadDatabase> = Arc::new(Other {
        database: crate::sqlite_database::MemorySqlite::new().await,
        hooks: StopsPastBound,
    });
    let (root, mut handles) = handles_over(1, &database);
    let repository = handles.remove(0);
    catch_up_in_publish(&repository).await.unwrap();
    (root, repository)
}

/// The answer of `O` on `request` through the public operation: the
/// success, or the status and body of the failure.
async fn answer<O: Operation>(repository: &Repository, request: Value) -> Value {
    let binding = LocalRepository::bind(repository.clone(), "memory").unwrap();
    let resolver: Arc<dyn ContextResolver> = Arc::new(binding);
    let selected = match O::CONTEXT {
        ContextKind::Scoped => RequestedContext::Scoped(serde_json::from_value(context()).unwrap()),
        ContextKind::Scope => RequestedContext::Scope(serde_json::from_value(context()).unwrap()),
        _ => panic!("{} is not a read of one scope", O::NAME),
    };
    let request = serde_json::from_value(request).unwrap();
    match invoke_authorized_typed::<O>(resolver, selected, request).await {
        Ok(success) => serde_json::to_value(success).unwrap(),
        Err(failure) => {
            let failure = frame_failure::<O>(failure);
            json!({ "status": failure.status_code(), "error": failure.error })
        }
    }
}

/// The typed page budget refusal, as a read answers it.
fn refusal() -> Value {
    json!({ "status": 409, "error": { "kind": "page_budget_exceeded" } })
}

/// Each family page past its work bound answers the page budget refusal
/// through its public operation.
#[tokio::test]
async fn each_family_page_past_its_work_bound_refuses_with_the_page_budget() {
    let (_root, repository) = repository().await;
    let page = json!({});
    let facts = json!({ "proposal_id": "proposal_none" });
    let answers = [
        (
            PageSources::NAME,
            answer::<PageSources>(&repository, page.clone()).await,
        ),
        (
            PageRequirements::NAME,
            answer::<PageRequirements>(&repository, page.clone()).await,
        ),
        (
            PageTopics::NAME,
            answer::<PageTopics>(&repository, page.clone()).await,
        ),
        (
            PageQuestions::NAME,
            answer::<PageQuestions>(&repository, page.clone()).await,
        ),
        (
            PageResolutions::NAME,
            answer::<PageResolutions>(&repository, page.clone()).await,
        ),
        (
            PageRules::NAME,
            answer::<PageRules>(&repository, page.clone()).await,
        ),
        (
            PageDiscussionContainers::NAME,
            answer::<PageDiscussionContainers>(&repository, page.clone()).await,
        ),
        (
            PageMessages::NAME,
            answer::<PageMessages>(&repository, page.clone()).await,
        ),
        (
            PageContributions::NAME,
            answer::<PageContributions>(&repository, page.clone()).await,
        ),
        (
            PageSynthesisPackets::NAME,
            answer::<PageSynthesisPackets>(&repository, page.clone()).await,
        ),
        (
            PageProposals::NAME,
            answer::<PageProposals>(&repository, page.clone()).await,
        ),
        (
            PageAssertions::NAME,
            answer::<PageAssertions>(&repository, page.clone()).await,
        ),
        (
            PageDispositions::NAME,
            answer::<PageDispositions>(&repository, page.clone()).await,
        ),
        (
            PageProposalAssertions::NAME,
            answer::<PageProposalAssertions>(&repository, facts.clone()).await,
        ),
        (
            PageProposalDispositions::NAME,
            answer::<PageProposalDispositions>(&repository, facts).await,
        ),
        (
            PageVerificationBindings::NAME,
            answer::<PageVerificationBindings>(&repository, page).await,
        ),
    ];
    for (operation, answer) in answers {
        assert_eq!(answer, refusal(), "{operation}");
    }
}

/// Each page of the read operations port past its work bound answers the
/// page budget refusal, as `read-document` does.
#[tokio::test]
async fn each_port_page_past_its_work_bound_refuses_with_the_page_budget() {
    let (_root, repository) = repository().await;
    let pages: Vec<_> = READ_OPERATIONS
        .iter()
        .copied()
        .filter(|operation| operation.starts_with("page-"))
        .collect();
    assert_eq!(pages.len(), 6, "{pages:?}");
    for operation in pages {
        assert_eq!(
            read(&repository, operation, json!({})).await,
            refusal(),
            "{operation}"
        );
    }
    let document = read(&repository, "read-document", json!({ "id": "req_expiry" })).await;
    assert_eq!(document, refusal());
}
