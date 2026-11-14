//! The answers of the read operations over memory storage, after the
//! embedder's catch-up and between two catch-ups.

use crate::{context, ids, read, read_in, repository, requirement, stamp, with_policy};
use provenance_store::cache::catch_up_in_publish;
use provenance_store::storage::memory::MemorySectionLock;
use provenance_store::storage::SectionLock as _;
use serde_json::{json, Value};

/// The stamp policy of a read that asks for no freshness step.
#[cfg(feature = "host")]
const DEFAULT_POLICY: &str = "catch_up";
#[cfg(not(feature = "host"))]
const DEFAULT_POLICY: &str = "annotate_only";

/// The checked answers of [`reads`] after the first catch-up, without their
/// stamps. They are the same with and without `host`.
const PINNED: &str = include_str!("../fixtures/read_operations_on_memory.json");

/// One call of each read operation.
fn reads() -> Vec<(&'static str, Value)> {
    let parent = json!({ "node_type": "requirement", "node_id": "req_expiry" });
    vec![
        ("page-sources", json!({})),
        ("page-requirements", json!({ "limit": 1 })),
        ("page-resolutions", json!({})),
        ("page-rules", json!({})),
        ("page-topics", json!({})),
        ("page-questions", json!({})),
        (
            "get-reviewed-resource",
            json!({ "record_kind": "rule", "id": "rule_expiry" }),
        ),
        ("read-document", json!({ "id": "req_expiry" })),
        ("review-discussions", json!({ "parent": parent })),
        (
            "review-discussion-messages",
            json!({
                "parent": parent,
                "selector": { "kind": "discussion", "discussion_id": "discussion_none" },
            }),
        ),
    ]
}

/// Fails with each line of `observed` that differs from the checked file,
/// and with the whole observed text.
fn assert_pinned(observed: &str) {
    let expected = PINNED.trim_end();
    if observed == expected {
        return;
    }
    let (observed_lines, expected_lines): (Vec<_>, Vec<_>) =
        (observed.lines().collect(), expected.lines().collect());
    let mut diff = Vec::new();
    for index in 0..observed_lines.len().max(expected_lines.len()) {
        let (left, right) = (expected_lines.get(index), observed_lines.get(index));
        if left != right {
            diff.push(format!("line {}:", index + 1));
            diff.extend(left.map(|line| format!("- {line}")));
            diff.extend(right.map(|line| format!("+ {line}")));
        }
    }
    panic!(
        "the answers differ from tests/fixtures/read_operations_on_memory.json \
         (- expected, + observed):\n{}\n=== observed answers\n{observed}\n=== end",
        diff.join("\n")
    );
}

/// The answers of the read operations, after the embedder's catch-up, are
/// the same with and without `host`; only the stamp policy differs. Each
/// success has a stamp at the serial of the catch-up.
#[tokio::test]
async fn the_reads_answer_alike_with_and_without_host() {
    let (_root, repository, _) = repository().await;
    let report = catch_up_in_publish(&repository).await.unwrap();
    assert!(report.rebuilt, "the first pass builds the projection");

    let mut answers = Vec::new();
    for (operation, request) in reads() {
        let mut answer = read(&repository, operation, request).await;
        let stamp = stamp(&mut answer);
        if answer.get("error").is_none() {
            assert_eq!(stamp["policy"], DEFAULT_POLICY, "{operation}: {stamp}");
            assert_eq!(stamp["serial"], report.serial, "{operation}: {stamp}");
        } else {
            assert_eq!(stamp, Value::Null, "{operation}: {answer}");
        }
        answers.push(json!({ "operation": operation, "answer": answer }));
    }
    assert_pinned(&serde_json::to_string_pretty(&answers).unwrap());
}

/// Without `host` a read before any catch-up refuses as an `annotate_only`
/// read refuses a database that holds no current schema: `schema_behind`,
/// 409. With `host` the read builds the projection itself.
#[tokio::test]
async fn a_read_before_any_catch_up_answers_as_its_build_says() {
    let (_root, repository, _) = repository().await;
    let mut answer = read(&repository, "page-requirements", json!({})).await;
    let stamp = stamp(&mut answer);
    #[cfg(not(feature = "host"))]
    {
        assert!(stamp.is_null(), "{answer}");
        assert_eq!(answer["status"], 409, "{answer}");
        assert_eq!(answer["error"]["kind"], "schema_behind", "{answer}");
    }
    #[cfg(feature = "host")]
    {
        assert_eq!(stamp["policy"], "catch_up", "{stamp}");
        assert_eq!(ids(&answer), ["req_expiry"], "{answer}");
    }
}

/// A publish that the embedder follows with its catch-up shows in the next
/// read. Without `host` a read between the two answers at the serial of the
/// last catch-up, and its stamp says that no catch-up ran.
#[tokio::test]
async fn a_publish_then_the_catch_up_shows_in_the_next_read() {
    let (_root, repository, _) = repository().await;
    let first = catch_up_in_publish(&repository).await.unwrap();

    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();
    let mut between = read(&repository, "page-requirements", json!({})).await;
    let stamp_between = stamp(&mut between);
    assert_eq!(stamp_between["policy"], DEFAULT_POLICY, "{stamp_between}");
    #[cfg(not(feature = "host"))]
    {
        assert_eq!(stamp_between["serial"], first.serial, "{stamp_between}");
        assert_eq!(ids(&between), ["req_expiry"], "{between}");
    }
    #[cfg(feature = "host")]
    {
        assert_eq!(stamp_between["serial"], first.serial + 1, "{stamp_between}");
        assert_eq!(ids(&between), ["req_expiry", "req_owner"], "{between}");
    }

    let second = catch_up_in_publish(&repository).await.unwrap();
    assert_eq!(second.serial, first.serial + 1);
    let mut after = read(&repository, "page-requirements", json!({})).await;
    let stamp_after = stamp(&mut after);
    assert_eq!(stamp_after["serial"], second.serial, "{stamp_after}");
    assert_eq!(ids(&after), ["req_expiry", "req_owner"], "{after}");
}

/// The in-process port runs the calls of the target name of its binding,
/// and refuses a call that names another target, as the router does.
#[tokio::test]
async fn a_call_that_names_another_target_refuses() {
    let (_root, repository, _) = repository().await;
    catch_up_in_publish(&repository).await.unwrap();
    let other = json!({ "repository": "other", "scope": "default" });
    let answer = read_in(&repository, other, "page-requirements", json!({})).await;
    assert_eq!(answer["status"], 404, "{answer}");
    assert_eq!(answer["error"]["kind"], "unknown_target", "{answer}");
    let own = read_in(&repository, context(), "page-requirements", json!({})).await;
    assert_eq!(ids(&own), ["req_expiry"], "{own}");
}

/// A `refuse_stale` read over memory storage hashes the storage: it answers
/// after the embedder's catch-up, and refuses after a publish that no
/// catch-up followed, with the moved unit.
#[tokio::test]
async fn a_refuse_stale_read_hashes_the_storage() {
    let (_root, repository, _) = repository().await;
    let caught_up = catch_up_in_publish(&repository).await.unwrap();
    let refusing = with_policy("refuse_stale");

    let mut fresh = read_in(
        &repository,
        refusing.clone(),
        "page-requirements",
        json!({}),
    )
    .await;
    let stamp = stamp(&mut fresh);
    assert_eq!(stamp["policy"], "refuse_stale", "{fresh}");
    assert_eq!(stamp["serial"], caught_up.serial, "{stamp}");
    assert_eq!(ids(&fresh), ["req_expiry"], "{fresh}");

    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();
    let stale = read_in(&repository, refusing, "page-requirements", json!({})).await;
    assert_eq!(stale["status"], 409, "{stale}");
    assert_eq!(stale["error"]["kind"], "stale", "{stale}");
    assert_eq!(stale["error"]["serial"], caught_up.serial, "{stale}");
    let moved: Vec<&Value> = stale["error"]["moved"].as_array().unwrap().iter().collect();
    assert!(
        moved.iter().any(|unit| unit["unit"] == "scope:default"),
        "{stale}"
    );
}

/// A memory section lock made by `refusing` refuses an acquire of a held
/// section at once, in each build: no thread waits for the section.
#[test]
fn a_refusing_memory_section_lock_refuses_a_held_section() {
    let lock = MemorySectionLock::refusing();
    let held = lock.acquire().unwrap();
    let other = lock.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        sender
            .send(other.acquire().map(drop).map_err(|error| error.to_string()))
            .unwrap()
    });
    let refused = receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the second acquire waits for the held section");
    assert_eq!(
        refused,
        Err("the section is held, and this section lock refuses instead of waiting".to_owned())
    );
    drop(held);
    assert!(lock.acquire().is_ok());
}
