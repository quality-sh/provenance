//! The served operations over the test stores, and the timing report.
//!
//! The baselines are gone. The pinned answers file holds the bytes the
//! `get`, `search`, and `stale` answers must keep; the count test
//! compares the projection with the canonical readers over this
//! repository's own state; and the every-operation check answers all
//! eight operations over that store. One ignored test times the served
//! side over every store and prints the rows; it is a report, not a
//! gate:
//!
//! `cargo test -p provenance-store --release -- --ignored timing_comparison_rows --nocapture`

pub mod requests;
pub mod test_stores;
mod timing;

use crate::cache::{catch_up_state, open_cache};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader::{self, ReadSnapshot};
use provenance_core::model::ProjectionRow;
use provenance_core::{
    Boundary, Domain, ImplementationBinding, Question, Requirement, RequirementReview, Resolution,
    Rule, Source, Topic, VerificationBinding,
};
use requests::Request;
use serde_json::{json, Value};
use std::time::Instant;
use test_stores::TestStore;

/// Fields the served answer carries beside the answer bytes; the pinned
/// answers file holds the answers without them.
const ADDITIVE_FIELDS: [&str; 7] = [
    "stamp",
    "freshness_error",
    "implementation_bindings_has_more",
    "verification_bindings_has_more",
    "verification_runs_has_more",
    "reviews_has_more",
    "scan_cut",
];

pub fn strip_additive(value: &mut Value) {
    if let Some(map) = value.as_object_mut() {
        for field in ADDITIVE_FIELDS {
            map.remove(field);
        }
    }
}

fn settle<T: serde::Serialize>(answer: anyhow::Result<T>) -> Value {
    match answer {
        Ok(result) => serde_json::to_value(result).unwrap(),
        Err(error) => json!({ "error": error.to_string() }),
    }
}

/// One request through the reader under the given policy, as a value; a
/// refusal becomes `{"error": ..}` so the pinned file compares one way.
pub async fn served_value(store: &TestStore, request: &Request, policy: ReadPolicy) -> Value {
    match served_stamped(store, request, policy).await {
        Ok(stamped) => stamped.result,
        Err(error) => json!({ "error": error.to_string() }),
    }
}

pub async fn served_stamped(
    store: &TestStore,
    request: &Request,
    policy: ReadPolicy,
) -> anyhow::Result<provenance_core::protocol::Stamped<Value>> {
    use super::super::{evidence, impact, records, stale, symbols, walk};
    let scope = store.scope.clone();
    let request = request.clone();
    reader::answer(&store.root, &store.scope, policy, move |ctx| {
        Box::pin(async move {
            Ok(match request {
                Request::Get(query) => settle(records::get(ctx, query).await),
                Request::Search(query) => settle(records::search(ctx, query).await),
                Request::Neighbors(query) => settle(walk::neighbors(ctx, query).await),
                Request::Trace(query) => settle(walk::trace(ctx, query).await),
                Request::Impact(query) => settle(impact::impact(ctx, query).await),
                Request::Evidence(query) => settle(evidence::evidence(ctx, query).await),
                Request::Stale(query) => settle(stale::stale(ctx, &scope, query)),
                Request::ResolveSymbol(query) => settle(symbols::resolve(ctx, query).await),
            })
        })
    })
    .await
}

/// The served side: the operations through the reader. The freshness step
/// stays out of the number: the store is caught up once, then every read
/// runs under `annotate_only`. The scan is the one `prepare` took before
/// the clock started.
async fn served_answer(store: &TestStore, request: &Request) -> Value {
    let policy = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);
    let mut answer = served_value(store, request, policy).await;
    strip_additive(&mut answer);
    answer
}

/// One catch-up so the served side has a projection, then the scan the
/// served side reads from.
async fn prepare(store: &TestStore) -> (f64, f64) {
    let layout = store.layout();
    let started = Instant::now();
    catch_up_state(&layout).await.unwrap();
    let rebuild_ms = timing::elapsed_ms(started);
    // The steady-state pass, which every read under `catch_up` pays.
    let started = Instant::now();
    catch_up_state(&layout).await.unwrap();
    let catch_up_ms = timing::elapsed_ms(started);
    let scans = provenance_scanner::scan_path(&store.root).unwrap();
    crate::test_probes::set_test_scan(Some(scans));
    (rebuild_ms, catch_up_ms)
}

/// Times every case in the request set over one store and prints the
/// rows.
async fn print_timings(store: TestStore) {
    let (rebuild_ms, _) = prepare(&store).await;
    let layout = store.layout();
    let mut catch_up_samples = Vec::new();
    for _ in 0..timing::RUNS {
        let started = Instant::now();
        catch_up_state(&layout).await.unwrap();
        catch_up_samples.push(timing::elapsed_ms(started));
    }
    let catch_up_ms = timing::median(&mut catch_up_samples);
    let started = Instant::now();
    provenance_scanner::scan_path(&store.root).unwrap();
    let scan_ms = timing::elapsed_ms(started);
    let mut rows = Vec::new();
    for request in &requests::for_store(&store).await {
        served_answer(&store, request).await;
        let mut served_samples = Vec::new();
        for _ in 0..timing::RUNS {
            let started = Instant::now();
            served_answer(&store, request).await;
            served_samples.push(timing::elapsed_ms(started));
        }
        rows.push(timing::Row {
            operation: request.operation(),
            request: request.describe(),
            served_ms: timing::median(&mut served_samples),
        });
    }
    crate::test_probes::set_test_scan(None);
    timing::print_rows(store.name, &rows, scan_ms, rebuild_ms, catch_up_ms);
}

/// Every operation answers with a stamp over a copy of this repository's
/// own state. `stale` and the diff half of `evidence` need a commit
/// range, so the copy gets one; when git is not on the path the test
/// says it skipped.
#[tokio::test]
async fn every_operation_answers_over_the_repository_state() {
    let mut store = test_stores::repository_state();
    let Some(base) = test_stores::git_commit(&store.root, "state") else {
        println!("skipped: git is not on the path, so the stale cases cannot be built");
        return;
    };
    store.base_commit = Some(base);
    prepare(&store).await;
    let policy = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);
    let mut answered: Vec<&'static str> = Vec::new();
    for request in &requests::for_store(&store).await {
        let operation = request.operation();
        let answer = served_stamped(&store, request, policy)
            .await
            .unwrap_or_else(|error| {
                panic!("{operation} {} must answer: {error}", request.describe())
            });
        assert!(
            answer.freshness_error.is_none(),
            "{operation} must answer without a freshness error, got {:?}",
            answer.freshness_error
        );
        assert!(
            !answer.stamp.digest.is_empty(),
            "{operation} must stamp its answer"
        );
        if !answered.contains(&operation) {
            answered.push(operation);
        }
    }
    crate::test_probes::set_test_scan(None);
    assert_eq!(
        answered.len(),
        8,
        "every operation must appear in the request set, got {answered:?}"
    );
}

/// The one comparison against canonical bytes over a store of real size:
/// for every kind and integration table, the projection holds exactly the
/// rows the canonical reader lists over a copy of this repository's own
/// state, retired records included.
#[tokio::test]
async fn projection_counts_match_canonical_over_the_repository_state() {
    let store = test_stores::repository_state();
    let state = store.state_store();
    let scope = &store.scope;
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    let snapshot = reader::ReadSnapshot::open(&pool, scope)
        .await
        .unwrap()
        .expect("a revision");
    assert_count::<Source>(&snapshot, state.list_sources(scope).unwrap().len()).await;
    assert_count::<Requirement>(&snapshot, state.list_requirements(scope).unwrap().len()).await;
    assert_count::<Resolution>(&snapshot, state.list_resolutions(scope).unwrap().len()).await;
    assert_count::<Rule>(&snapshot, state.list_rules(scope).unwrap().len()).await;
    assert_count::<Topic>(&snapshot, state.list_topics(scope).unwrap().len()).await;
    assert_count::<Question>(&snapshot, state.list_questions(scope).unwrap().len()).await;
    assert_count::<Domain>(&snapshot, state.list_domains(scope).unwrap().len()).await;
    assert_count::<Boundary>(&snapshot, state.list_boundaries(scope).unwrap().len()).await;
    assert_count::<ImplementationBinding>(
        &snapshot,
        state.list_implementation_bindings(scope).unwrap().len(),
    )
    .await;
    assert_count::<VerificationBinding>(
        &snapshot,
        state.list_verification_bindings(scope).unwrap().len(),
    )
    .await;
    assert_count::<RequirementReview>(
        &snapshot,
        state.list_requirement_reviews(scope).unwrap().len(),
    )
    .await;
    drop(snapshot);
    pool.close().await;
}

async fn assert_count<K: ProjectionRow>(snapshot: &ReadSnapshot, canonical: usize) {
    let counted = snapshot.table::<K>().count().await.unwrap();
    let canonical = i64::try_from(canonical).expect("a row count fits an i64");
    assert_eq!(
        counted,
        canonical,
        "{}: the projection must hold every canonical row, retired included",
        K::TABLE
    );
}

/// The timing report over every store. Run it by hand:
/// `cargo test -p provenance-store --release -- --ignored timing_comparison_rows --nocapture`
#[tokio::test]
#[ignore = "a report, not a gate; run by hand with --ignored"]
async fn timing_comparison_rows() {
    print_timings(test_stores::seeded_queries()).await;
    for store in test_stores::cache_fixtures() {
        print_timings(store).await;
    }
    print_timings(test_stores::repository_state()).await;
}
