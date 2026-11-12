//! A repository whose read database is not the file.
//!
//! The row readers run on the port, so its reads answer as the file's do.
//! The catch-up pass, the schema check of `annotate_only` and
//! `refuse_stale`, and the discussion reads still run `SQLx` on the file,
//! so each of them refuses another adapter with a fault and does not
//! panic. No Rule: the limit ends with the pull request that moves them.

use super::super::comparison::test_stores::{self, TestStore};
use super::get_through;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::read_database::contract::Other;
use crate::read_database::file::FileReadDatabase;
use crate::repository::{Repository, RepositoryIdentity};
use crate::settings::Settings;
use crate::storage::local::{LocalSectionLock, LocalStateStorage};
use provenance_core::protocol::StampPolicy;
use provenance_core::threads::{DiscussionListQuery, DiscussionStatusFilter};
use provenance_core::NodeType;
use std::sync::Arc;

/// The repository of `store` over the other adapter.
fn over_the_other_adapter(store: &TestStore) -> Repository {
    let layout = store.layout();
    Repository::with_storage(
        RepositoryIdentity::new(store.root.clone(), store.scope.clone(), Settings::default()),
        Arc::new(LocalStateStorage::new(layout.clone())),
        Arc::new(LocalSectionLock::new(layout.clone())),
        Some(Arc::new(Other(FileReadDatabase::new(layout)))),
    )
    .unwrap()
}

fn assert_fault(failed: &anyhow::Error, text: &str) {
    assert_eq!(format!("{failed:#}"), text);
    assert!(
        failed.downcast_ref::<crate::refusal::Fault>().is_some(),
        "not a fault: {failed:#}"
    );
}

#[tokio::test]
async fn a_read_of_another_adapter_runs_the_moved_readers() {
    let store = test_stores::seeded_queries();
    let stored = get_through(&store, ReadPolicy::default()).await.unwrap();
    let repository = over_the_other_adapter(&store);
    let read = |policy| {
        let repository = repository.clone();
        async move {
            crate::operations::reader::answer(
                &repository,
                ReadPolicy::with_freshness(policy),
                |ctx| {
                    Box::pin(async move {
                        crate::operations::queries::records::get(
                            ctx,
                            super::get_query("req_overtime"),
                        )
                        .await
                    })
                },
            )
            .await
        }
    };

    // Under catch-up, the pass refuses to write the other adapter, and the
    // read answers at the stored serial as the file does.
    let caught = read(FreshnessPolicy::CatchUp).await.unwrap();
    assert_eq!(caught.stamp.policy, StampPolicy::CatchUpFailed);
    assert_eq!(caught.stamp.serial, stored.stamp.serial);
    let because = caught.freshness_error.as_deref().unwrap_or_default();
    assert!(
        because.contains("the projection writes the file adapter"),
        "{because}"
    );
    assert_eq!(
        serde_json::to_value(&caught.result).unwrap(),
        serde_json::to_value(&stored.result).unwrap()
    );

    // The schema check of annotate_only and refuse_stale reads the file
    // adapter.
    for policy in [FreshnessPolicy::AnnotateOnly, FreshnessPolicy::RefuseStale] {
        let failed = read(policy).await.unwrap_err();
        assert_fault(&failed, "the schema check reads the file adapter");
    }

    // A discussion read refuses at its first SQLx statement.
    let failed = crate::review::read_discussion_list(
        &repository,
        ReadPolicy::with_freshness(FreshnessPolicy::CatchUp),
        DiscussionListQuery {
            parent: None,
            allowed_parent_kinds: vec![NodeType::Requirement],
            status: DiscussionStatusFilter::All,
            limit: 10,
            cursor: None,
        },
    )
    .await
    .unwrap_err();
    assert_fault(
        &failed,
        "the discussion reads read only the read database file under .provenance/cache",
    );

    // The file keeps the revision that it had.
    let after = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap();
    assert_eq!(after.stamp.serial, stored.stamp.serial);
}
