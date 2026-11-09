//! `evidence` reads its bindings and reviews from the projection, says
//! which of its four lists a limit cut, and answers only the reviews
//! still open.

use super::comparison::requests;
use super::comparison::test_stores::TestStore;
use crate::operations::catalog;
use crate::operations::queries;
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::failure::OperationFailure;
use provenance_macros::verifies;
use serde_json::json;
use std::sync::Arc;

/// Two implementations, three verifications, and two open reviews share
/// a limit of two. Only the verification list is cut.
#[tokio::test]
#[verifies("rule_evidence_flags_each_cut_list", examples)]
async fn evidence_reports_which_list_was_cut() {
    let store = TestStore::pinned();
    let mut request = requests::evidence("rule_overtime_001", None);
    request.limit = 2;
    let answer = queries::evidence(&store.repository(), ReadPolicy::default(), request.clone())
        .await
        .unwrap()
        .result;
    assert_eq!(answer.implementation_bindings.len(), 2);
    assert!(!answer.implementation_bindings_has_more);
    assert_eq!(answer.verification_bindings.len(), 2);
    assert!(answer.verification_bindings_has_more);
    assert_eq!(
        answer.verification_runs,
        [] as [provenance_core::VerificationRun; 0]
    );
    assert!(!answer.verification_runs_has_more);
    assert_eq!(answer.reviews.len(), 2);
    assert!(!answer.reviews_has_more);
    assert!(answer.has_more, "the top-level flag is the OR of the four");
}

/// `review_b` was cleared by a run; it is neither answered nor counted
/// toward `review_required`, and the review table stands behind the
/// answer.
#[tokio::test]
async fn a_cleared_review_is_not_open() {
    let store = TestStore::pinned();
    let answer = queries::evidence(
        &store.repository(),
        ReadPolicy::default(),
        requests::evidence("rule_overtime_001", None),
    )
    .await
    .unwrap();
    let reviews: Vec<&str> = answer
        .result
        .reviews
        .iter()
        .map(|review| review.id.as_str())
        .collect();
    assert_eq!(reviews, ["review_a", "review_c"]);
    assert!(answer.result.review_required);
    assert_eq!(
        answer.stamp.attested,
        [
            "implementation_bindings",
            "requirement_reviews",
            "verification_bindings"
        ]
    );
    assert_eq!(answer.stamp.live, ["scanned_sites", "verification_runs"]);
}

/// A host that opens the graph but gives no access to the repository
/// files: it refuses each operation that names `RepositoryFiles`.
struct NoRepositoryFiles(camino::Utf8PathBuf, String);

impl catalog::ContextResolver for NoRepositoryFiles {
    fn authorize(
        &self,
        _: &'static str,
        _: &catalog::RequestedContext,
    ) -> Result<(), OperationFailure> {
        Ok(())
    }

    fn prepare(
        &self,
        _: &'static str,
        _: catalog::RequestedContext,
        needs: catalog::ExecutionNeeds,
    ) -> Result<catalog::PreparedContext, OperationFailure> {
        if needs.contains(&catalog::ExecutionNeed::RepositoryFiles) {
            return Err(OperationFailure::UnavailableNeeds);
        }
        Ok(catalog::PreparedContext::read(catalog::PreparedRead {
            repository: crate::repository::Repository::open(&self.0, &self.1).unwrap(),
            policy: ReadPolicy::default(),
            requested_target: "test".into(),
            external: false,
        }))
    }
}

async fn call_without_files<O: catalog::Operation>(
    store: &TestStore,
    request: serde_json::Value,
) -> Result<serde_json::Value, provenance_core::protocol::failure::ErasedFailure> {
    crate::test_support::call::<O, _>(
        json!({
            "context": {"repository": "test", "scope": store.scope.as_str()},
            "request": request
        }),
        catalog::Direct(Arc::new(NoRepositoryFiles(
            store.root.clone(),
            store.scope.as_str().to_owned(),
        ))),
    )
    .await
}

/// The evidence read scans the working tree as impact does, so a host
/// without the repository files refuses it before it runs, with the
/// failure that impact gives.
#[tokio::test]
#[verifies("rule_evidence_gives_scanned_sites", examples)]
async fn a_host_without_repository_files_refuses_evidence_as_it_refuses_impact() {
    let store = TestStore::pinned();

    let impact = call_without_files::<catalog::Impact>(
        &store,
        json!({"id": "rule_overtime_001", "limit": 50}),
    )
    .await
    .unwrap_err();
    let evidence = call_without_files::<catalog::Evidence>(
        &store,
        json!({"rule": "rule_overtime_001", "limit": 50}),
    )
    .await
    .expect_err("evidence reads the working tree, so it needs the repository files");

    assert_eq!(impact.status_code(), 503);
    assert_eq!(evidence.status_code(), impact.status_code());
    assert_eq!(evidence.error, impact.error);
}
