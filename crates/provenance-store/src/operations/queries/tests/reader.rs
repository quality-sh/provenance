//! The reader entry: every answer carries a stamp at the stored serial,
//! the handles put their words on it, and a canonical edit moves the
//! serial under catch-up.

mod freshness;
mod guard;

use super::differential::corpus::{self, Corpus};
use super::differential::requests;
use crate::cache::{open_cache, ProjectionFamily};
use crate::operations::queries::{self, records};
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::{answer, ReadSnapshot};
use crate::operations::stamp::{seal, READ_DERIVATION};
use provenance_core::protocol::{
    GetQuery, GetResult, ImpactQuery, ResolveSymbolQuery, StaleQuery, Stamp, StampPolicy, Stamped,
    SDK_PROTOCOL_VERSION,
};
use provenance_core::NodeType;

/// The latest revision serial and the instance id, read directly.
async fn stored(corpus: &Corpus) -> (i64, String) {
    let pool = open_cache(&corpus.layout()).await.unwrap();
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
    corpus: &Corpus,
    policy: ReadPolicy,
) -> anyhow::Result<Stamped<GetResult>> {
    let scope = corpus.scope.clone();
    answer(&corpus.root, &corpus.scope, policy, move |ctx| {
        Box::pin(async move { records::get(ctx, &scope, get_query("req_overtime")) })
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
async fn graph_stamps(corpus: &Corpus) -> Vec<(&'static str, Stamp)> {
    let repo = || Some(corpus.root.clone());
    let scope = &corpus.scope;
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

/// The stamps of the four operations that read a live half beside
/// canonical state.
async fn evidence_stamps(corpus: &Corpus, base: &str) -> Vec<(&'static str, Stamp)> {
    let repo = || Some(corpus.root.clone());
    let scope = &corpus.scope;
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
    let corpus = corpus::seeded_queries();
    let base = corpus
        .base_commit
        .clone()
        .expect("a commit to diff against");
    let mut stamps = graph_stamps(&corpus).await;
    stamps.extend(evidence_stamps(&corpus, &base).await);
    assert_eq!(stamps.len(), 8);

    let (serial, instance_id) = stored(&corpus).await;
    for (operation, stamp) in &stamps {
        assert_eq!(stamp.serial, serial, "{operation} serial");
        assert_eq!(stamp.instance_id, instance_id, "{operation} instance");
        assert_eq!(stamp.derivation, READ_DERIVATION, "{operation} derivation");
        assert_eq!(stamp.policy, StampPolicy::CatchUp, "{operation} policy");
        assert!(stamp.digest.starts_with("sha256:"), "{operation} digest");
        let (attested, live) = words(stamp);
        assert!(
            attested.is_empty(),
            "{operation} attests nothing before its flip"
        );
        let expected: &[&str] = match *operation {
            "impact" | "resolve_symbol" => &["canonical", "scanned_sites"],
            "evidence" => &["canonical", "diff", "verification_runs"],
            "stale" => &["canonical", "diff"],
            _ => &["canonical"],
        };
        assert_eq!(live, expected, "{operation} live words");
    }
}

#[tokio::test]
async fn evidence_without_a_base_lists_no_diff() {
    let corpus = corpus::seeded_queries();
    let answer = queries::evidence(
        Some(corpus.root.clone()),
        &corpus.scope,
        requests::evidence("rule_overtime", None),
    )
    .await
    .unwrap();
    assert!(answer.result.stale.is_none());
    assert_eq!(words(&answer.stamp).1, ["canonical", "verification_runs"]);
}

#[tokio::test]
async fn a_table_handle_puts_its_word_in_attested() {
    let corpus = corpus::seeded_queries();
    crate::cache::catch_up_state(&corpus.layout())
        .await
        .unwrap();
    let stamped = answer(&corpus.root, &corpus.scope, ReadPolicy::default(), |ctx| {
        Box::pin(async move {
            let requirements = ctx.snapshot().table(ProjectionFamily::Requirements);
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

    let pool = open_cache(&corpus.layout()).await.unwrap();
    let snapshot = ReadSnapshot::open(&pool, &corpus.scope)
        .await
        .unwrap()
        .expect("a revision");
    let _rules = snapshot.table(ProjectionFamily::Rules);
    let context = crate::operations::reader::ReadContext::for_test(snapshot, &corpus.root);
    let _store = context
        .live(crate::operations::reader::Live::Canonical)
        .store();
    let stamp = seal(context, StampPolicy::AnnotateOnly);
    assert_eq!(words(&stamp), (vec!["rules"], vec!["canonical"]));
    pool.close().await;
}

#[tokio::test]
async fn a_canonical_edit_advances_the_serial_under_catch_up() {
    let corpus = corpus::seeded_queries();
    let first = get_through(&corpus, ReadPolicy::default()).await.unwrap();
    crate::cache::tests::fixtures::create_requirement(
        &corpus.store(),
        &corpus.scope,
        "req_second",
        provenance_core::RequirementStatus::Active,
    );
    let second = get_through(&corpus, ReadPolicy::default()).await.unwrap();
    assert_eq!(second.stamp.serial, first.stamp.serial + 1);
    assert_ne!(second.stamp.digest, first.stamp.digest);
    assert_eq!(second.stamp.instance_id, first.stamp.instance_id);
    assert!(second.freshness_error.is_none());
}
