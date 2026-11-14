//! The embedder's catch-up: one pass at a time, and the state after a pass
//! that fails once the publish committed.

use crate::{ids, read, read_in, repository, requirement, stamp, with_policy, Switch};
use provenance_store::cache::{catch_up_in_publish, CatchUpRunning};
use provenance_store::repository::Repository;
use serde_json::json;
use std::future::Future;
use std::time::Duration;

/// The answer of `second`, which must end while `first` holds its write of
/// the read database. Then `first` ends.
async fn while_held<A: Send, B: Send>(
    switch: &Switch,
    first: impl Future<Output = A> + Send,
    second: impl Future<Output = B> + Send,
) -> (A, B) {
    switch.hold();
    tokio::pin!(first);
    tokio::select! {
        _ = &mut first => panic!("the held write ended before its release"),
        () = switch.entered() => {}
    }
    let second = tokio::time::timeout(Duration::from_secs(10), second)
        .await
        .expect("the second call waits for the held pass");
    switch.release();
    (first.await, second)
}

/// The serial and the Requirement ids of an `annotate_only` read, which
/// writes nothing in each build.
async fn stored(repository: &Repository) -> (serde_json::Value, Vec<String>) {
    let mut page = read_in(
        repository,
        with_policy("annotate_only"),
        "page-requirements",
        json!({}),
    )
    .await;
    let serial = stamp(&mut page)["serial"].clone();
    let ids = ids(&page).into_iter().map(str::to_owned).collect();
    (serial, ids)
}

/// A call of `catch_up_in_publish` while another pass of the same
/// repository writes its read database refuses at once with
/// `CatchUpRunning`, and writes nothing. The first pass then commits.
#[tokio::test]
async fn a_second_catch_up_at_once_refuses_and_writes_nothing() {
    let (_root, repository, switch) = repository().await;
    let first = catch_up_in_publish(&repository).await.unwrap();
    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();

    let (running, refused) = while_held(
        &switch,
        catch_up_in_publish(&repository),
        catch_up_in_publish(&repository.clone()),
    )
    .await;
    let refused = refused.unwrap_err();
    assert_eq!(
        refused.downcast_ref::<CatchUpRunning>(),
        Some(&CatchUpRunning),
        "{refused:#}"
    );
    assert_eq!(
        refused.to_string(),
        "another catch-up pass of this repository is running"
    );
    let running = running.unwrap();
    assert_eq!(running.serial, first.serial + 1);
    assert_eq!(
        stored(&repository).await,
        (
            json!(running.serial),
            vec!["req_expiry".into(), "req_owner".into()]
        )
    );
    assert!(
        !catch_up_in_publish(&repository)
            .await
            .unwrap()
            .revision_committed
    );
}

/// When the pass fails after the publish committed, the read database stays
/// at the serial of the last pass. Without `host` a read answers there,
/// stamped `annotate_only` with no freshness error, and a `refuse_stale`
/// read refuses `stale`. With `host` a `catch_up` read catches up itself.
/// The next call that succeeds catches the database up in both builds.
#[tokio::test]
async fn a_failed_catch_up_after_a_publish_leaves_the_last_serial() {
    let (_root, repository, switch) = repository().await;
    let first = catch_up_in_publish(&repository).await.unwrap();
    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();
    switch.fail();
    let failed = catch_up_in_publish(&repository).await.unwrap_err();
    assert_eq!(failed.to_string(), "the test failed this write");
    assert_eq!(
        stored(&repository).await,
        (json!(first.serial), vec!["req_expiry".to_owned()])
    );
    let stale = read_in(
        &repository,
        with_policy("refuse_stale"),
        "page-requirements",
        json!({}),
    )
    .await;
    assert_eq!(stale["error"]["kind"], "stale", "{stale}");

    let mut answer = read(&repository, "page-requirements", json!({})).await;
    let stamp = stamp(&mut answer);
    assert_eq!(
        answer.get("freshness_error"),
        Some(&json!(null)),
        "{answer}"
    );
    #[cfg(not(feature = "host"))]
    {
        assert_eq!(stamp["policy"], "annotate_only", "{stamp}");
        assert_eq!(stamp["serial"], first.serial, "{stamp}");
        assert_eq!(ids(&answer), ["req_expiry"], "{answer}");
    }
    #[cfg(feature = "host")]
    {
        assert_eq!(stamp["policy"], "catch_up", "{stamp}");
        assert_eq!(stamp["serial"], first.serial + 1, "{stamp}");
        assert_eq!(ids(&answer), ["req_expiry", "req_owner"], "{answer}");
    }

    catch_up_in_publish(&repository).await.unwrap();
    assert_eq!(
        stored(&repository).await,
        (
            json!(first.serial + 1),
            vec!["req_expiry".into(), "req_owner".into()]
        )
    );
}

/// With `host`, a read whose own catch-up starts while a pass of
/// `catch_up_in_publish` writes answers as a read whose catch-up failed: at
/// the stored serial, stamped `catch_up_failed`, with the refusal as its
/// freshness error.
#[cfg(feature = "host")]
#[tokio::test]
async fn a_read_during_the_embedders_pass_answers_as_a_failed_catch_up() {
    let (_root, repository, switch) = repository().await;
    let first = catch_up_in_publish(&repository).await.unwrap();
    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();

    let (running, mut answer) = while_held(
        &switch,
        catch_up_in_publish(&repository),
        read(&repository, "page-requirements", json!({})),
    )
    .await;
    let stamp = stamp(&mut answer);
    assert_eq!(stamp["policy"], "catch_up_failed", "{stamp}");
    assert_eq!(stamp["serial"], first.serial, "{stamp}");
    assert_eq!(
        answer["freshness_error"],
        "another catch-up pass of this repository is running"
    );
    assert_eq!(ids(&answer), ["req_expiry"], "{answer}");
    assert_eq!(running.unwrap().serial, first.serial + 1);
}

/// With `host`, a call of `catch_up_in_publish` while a read catches up
/// refuses at once with `CatchUpRunning`; the read's pass commits.
#[cfg(feature = "host")]
#[tokio::test]
async fn the_embedders_pass_during_a_reads_catch_up_refuses() {
    let (_root, repository, switch) = repository().await;
    let first = catch_up_in_publish(&repository).await.unwrap();
    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();

    let (mut answer, refused) = while_held(
        &switch,
        read(&repository, "page-requirements", json!({})),
        catch_up_in_publish(&repository),
    )
    .await;
    let refused = refused.unwrap_err();
    assert_eq!(
        refused.downcast_ref::<CatchUpRunning>(),
        Some(&CatchUpRunning)
    );
    let stamp = stamp(&mut answer);
    assert_eq!(stamp["policy"], "catch_up", "{stamp}");
    assert_eq!(stamp["serial"], first.serial + 1, "{stamp}");
}
