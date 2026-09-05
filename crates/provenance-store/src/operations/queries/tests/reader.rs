//! The reader entry: every answer carries a stamp at the stored serial,
//! the handles put their words on it, and a canonical edit moves the
//! serial under catch-up.

mod freshness;
mod guard;

use super::comparison::requests;
use super::comparison::test_stores::{self, TestStore};
use crate::cache::open_cache;
use crate::operations::queries::{self, records};
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::{answer, ReadSnapshot};
use crate::operations::stamp::{seal, READ_DERIVATION};
use provenance_core::protocol::{
    GetQuery, GetResult, ImpactQuery, ResolveSymbolQuery, StaleQuery, Stamp, StampPolicy, Stamped,
    SDK_PROTOCOL_VERSION,
};
use provenance_core::{NodeType, Requirement, Rule};

/// The latest revision serial and the instance id, read directly.
async fn stored(store: &TestStore) -> (i64, String) {
    let pool = open_cache(&store.layout()).await.unwrap();
    let serial: i64 = sqlx::query_scalar("SELECT MAX(serial) FROM projection_revision")
        .fetch_one(&pool)
        .await
        .unwrap();
    let instance_id: String = sqlx::query_scalar("SELECT instance_id FROM projection_instance")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;
    (serial, instance_id)
}

pub(super) fn get_query(id: &str) -> GetQuery {
    GetQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        node_type: NodeType::Requirement,
        id: id.into(),
        include_retired: false,
    }
}

/// One `get` through the reader under the given policy.
pub(super) async fn get_through(
    store: &TestStore,
    policy: ReadPolicy,
) -> anyhow::Result<Stamped<GetResult>> {
    answer(&store.root, &store.scope, policy, move |ctx| {
        Box::pin(async move { records::get(ctx, get_query("req_overtime")).await })
    })
    .await
}

fn words(stamp: &Stamp) -> (Vec<&str>, Vec<&str>) {
    (
        stamp.attested.iter().map(String::as_str).collect(),
        stamp.live.iter().map(String::as_str).collect(),
    )
}

/// The stamps of the four graph operations over `req_overtime`.
async fn graph_stamps(store: &TestStore) -> Vec<(&'static str, Stamp)> {
    let repo = || Some(store.root.clone());
    let scope = &store.scope;
    vec![
        (
            "get",
            queries::get(repo(), scope, get_query("req_overtime"))
                .await
                .unwrap()
                .stamp,
        ),
        (
            "search",
            queries::search(repo(), scope, requests::search("pay", Vec::new()))
                .await
                .unwrap()
                .stamp,
        ),
        (
            "neighbors",
            queries::neighbors(
                repo(),
                scope,
                requests::neighbors("req_overtime", false, 10),
            )
            .await
            .unwrap()
            .stamp,
        ),
        (
            "trace",
            queries::trace(repo(), scope, requests::trace("req_overtime", false, 10))
                .await
                .unwrap()
                .stamp,
        ),
    ]
}

/// The stamps of the four operations that read a live part beside
/// canonical state.
async fn evidence_stamps(store: &TestStore, base: &str) -> Vec<(&'static str, Stamp)> {
    let repo = || Some(store.root.clone());
    let scope = &store.scope;
    let version = Some(SDK_PROTOCOL_VERSION);
    vec![
        (
            "impact",
            queries::impact(
                repo(),
                scope,
                ImpactQuery {
                    protocol_version: version,
                    id: "req_overtime".into(),
                    node_type: None,
                    include_retired: false,
                    limit: 10,
                },
            )
            .await
            .unwrap()
            .stamp,
        ),
        (
            "evidence",
            queries::evidence(
                repo(),
                scope,
                requests::evidence("rule_overtime", Some(base.to_string())),
            )
            .await
            .unwrap()
            .stamp,
        ),
        (
            "stale",
            queries::stale(
                repo(),
                scope,
                StaleQuery {
                    protocol_version: version,
                    base: base.to_string(),
                    head: None,
                    rules: Vec::new(),
                    include_retired: false,
                    limit: 10,
                },
            )
            .await
            .unwrap()
            .stamp,
        ),
        (
            "resolve_symbol",
            queries::resolve_symbol(
                repo(),
                scope,
                ResolveSymbolQuery {
                    protocol_version: version,
                    file: "src/pay.rs".into(),
                    symbol: None,
                    line: None,
                    include_retired: false,
                    limit: 10,
                },
            )
            .await
            .unwrap()
            .stamp,
        ),
    ]
}

#[tokio::test]
async fn every_answer_carries_a_stamp_at_the_stored_serial() {
    let store = test_stores::seeded_queries();
    let base = store.base_commit.clone().expect("a commit to diff against");
    let mut stamps = graph_stamps(&store).await;
    stamps.extend(evidence_stamps(&store, &base).await);
    assert_eq!(stamps.len(), 8);

    let (serial, instance_id) = stored(&store).await;
    for (operation, stamp) in &stamps {
        assert_eq!(stamp.serial, serial, "{operation} serial");
        assert_eq!(stamp.instance_id, instance_id, "{operation} instance");
        assert_eq!(stamp.derivation, READ_DERIVATION, "{operation} derivation");
        assert_eq!(stamp.policy, StampPolicy::CatchUp, "{operation} policy");
        assert!(stamp.digest.starts_with("sha256:"), "{operation} digest");
        let (attested, live): (&[&str], &[&str]) = match *operation {
            "get" => (&["requirements"], &[]),
            "neighbors" | "trace" => (
                &[
                    "boundaries",
                    "domains",
                    "relations",
                    "requirements",
                    "sources",
                ],
                &[],
            ),
            "search" => (
                &[
                    "questions",
                    "requirements",
                    "resolutions",
                    "rules",
                    "sources",
                    "topics",
                ],
                &[],
            ),
            "impact" => (
                &["relations", "requirements", "rules", "sources"],
                &["scanned_sites"],
            ),
            "resolve_symbol" => (&[], &["canonical", "scanned_sites"]),
            "evidence" => (
                &[
                    "implementation_bindings",
                    "requirement_reviews",
                    "verification_bindings",
                ],
                &["canonical", "diff", "verification_runs"],
            ),
            "stale" => (&[], &["canonical", "diff"]),
            _ => (&[], &["canonical"]),
        };
        assert_eq!(
            words(stamp),
            (attested.to_vec(), live.to_vec()),
            "{operation} stamp words"
        );
    }
}

#[tokio::test]
async fn evidence_without_a_base_lists_no_diff() {
    let store = test_stores::seeded_queries();
    let answer = queries::evidence(
        Some(store.root.clone()),
        &store.scope,
        requests::evidence("rule_overtime", None),
    )
    .await
    .unwrap();
    assert!(answer.result.stale.is_none());
    assert_eq!(
        words(&answer.stamp).1,
        ["verification_runs"],
        "canonical is read for the diff half alone, so it is not listed either"
    );
}

#[tokio::test]
async fn a_table_handle_puts_its_word_in_attested() {
    let store = test_stores::seeded_queries();
    crate::cache::catch_up_state(&store.layout()).await.unwrap();
    let stamped = answer(&store.root, &store.scope, ReadPolicy::default(), |ctx| {
        Box::pin(async move {
            let requirements = ctx.snapshot().table::<Requirement>();
            let rows = requirements.count().await?;
            let relations = ctx.snapshot().relations().count().await?;
            Ok((rows, relations))
        })
    })
    .await
    .unwrap();
    assert_eq!(
        stamped.result,
        (1, 2),
        "one requirement; its domain and boundary rows"
    );
    assert_eq!(
        words(&stamped.stamp),
        (vec!["relations", "requirements"], Vec::new())
    );

    let pool = open_cache(&store.layout()).await.unwrap();
    let snapshot = ReadSnapshot::open(&pool, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    let _rules = snapshot.table::<Rule>();
    let context = crate::operations::reader::ReadContext::for_test(snapshot, &store.root);
    let _store = context
        .live(crate::operations::reader::Live::Canonical)
        .store();
    let stamp = seal(context, StampPolicy::AnnotateOnly);
    assert_eq!(words(&stamp), (vec!["rules"], vec!["canonical"]));
    pool.close().await;
}

#[tokio::test]
async fn a_canonical_edit_advances_the_serial_under_catch_up() {
    let store = test_stores::seeded_queries();
    let first = get_through(&store, ReadPolicy::default()).await.unwrap();
    crate::cache::tests::fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        "req_second",
        provenance_core::RequirementStatus::Active,
    );
    let second = get_through(&store, ReadPolicy::default()).await.unwrap();
    assert_eq!(second.stamp.serial, first.stamp.serial + 1);
    assert_ne!(second.stamp.digest, first.stamp.digest);
    assert_eq!(second.stamp.instance_id, first.stamp.instance_id);
    assert!(second.freshness_error.is_none());
}

/// `stale` resolves the commit range before it reads the store, so when
/// both are bad the range is the error.
#[tokio::test]
async fn a_bad_base_is_refused_before_the_store_is_read() {
    let store = test_stores::seeded_queries();
    // One healthy read materializes, so the later reads answer at the
    // stored serial and reach the operation.
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let shard = crate::shards::requirements_path(&store.layout(), &store.scope);
    std::fs::write(&shard, "not a record\n").unwrap();
    let refused = queries::stale(
        Some(store.root.clone()),
        &store.scope,
        StaleQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            base: "no_such_commit".into(),
            head: None,
            rules: Vec::new(),
            include_retired: false,
            limit: 10,
        },
    )
    .await
    .unwrap_err();
    let text = format!("{refused:#}");
    assert!(text.contains("rev-parse"), "{text}");
    let refused = queries::evidence(
        Some(store.root.clone()),
        &store.scope,
        requests::evidence("rule_overtime", Some("no_such_commit".into())),
    )
    .await
    .unwrap_err();
    let text = format!("{refused:#}");
    assert!(text.contains("rev-parse"), "{text}");
}
