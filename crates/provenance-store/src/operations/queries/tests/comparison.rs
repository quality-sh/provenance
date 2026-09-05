//! Runs each served operation and its baseline copy over the same store and
//! asserts the answers agree. One ignored test times both sides over every
//! store and prints the rows; it is a report, not a gate:
//!
//! `cargo test -p provenance-store --release -- --ignored timing_comparison_rows --nocapture`

pub mod requests;
pub mod test_stores;
mod timing;

use super::baseline;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader;
use provenance_scanner::FileScan;
use requests::Request;
use serde_json::{json, Value};
use std::time::Instant;
use test_stores::TestStore;

/// Fields the served side adds beside the answer; the baseline has none.
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

fn baseline_answer(store: &TestStore, request: &Request, scans: &[FileScan]) -> Value {
    let state = store.state_store();
    let (repo, scope) = (store.root.as_path(), &store.scope);
    match request.clone() {
        Request::Get(query) => settle(baseline::records::get(&state, scope, query)),
        Request::Search(query) => settle(baseline::records::search(&state, scope, query)),
        Request::Neighbors(query) => settle(baseline::walk::neighbors(&state, scope, query)),
        Request::Trace(query) => settle(baseline::walk::trace(&state, scope, query)),
        Request::Impact(query) => {
            settle(baseline::impact::impact(repo, &state, scope, query, scans))
        }
        Request::Evidence(query) => {
            settle(baseline::evidence::evidence(repo, &state, scope, query))
        }
        Request::Stale(query) => settle(baseline::stale::stale(repo, scope, query)),
        Request::ResolveSymbol(query) => settle(baseline::symbols::resolve(
            repo, &state, scope, query, scans,
        )),
    }
}

/// One request through the reader under the given policy, as a value; a
/// refusal becomes `{"error": ..}` so both sides compare the same way.
pub async fn served_value(store: &TestStore, request: &Request, policy: ReadPolicy) -> Value {
    use super::super::{evidence, impact, records, stale, symbols, walk};
    let scope = store.scope.clone();
    let request = request.clone();
    let served = reader::answer(&store.root, &store.scope, policy, move |ctx| {
        Box::pin(async move {
            Ok(match request {
                Request::Get(query) => settle(records::get(ctx, &scope, query)),
                Request::Search(query) => settle(records::search(ctx, &scope, query)),
                Request::Neighbors(query) => settle(walk::neighbors(ctx, &scope, query)),
                Request::Trace(query) => settle(walk::trace(ctx, &scope, query)),
                Request::Impact(query) => settle(impact::impact(ctx, &scope, query)),
                Request::Evidence(query) => settle(evidence::evidence(ctx, &scope, query)),
                Request::Stale(query) => settle(stale::stale(ctx, &scope, query)),
                Request::ResolveSymbol(query) => settle(symbols::resolve(ctx, &scope, query)),
            })
        })
    })
    .await;
    match served {
        Ok(stamped) => stamped.result,
        Err(error) => json!({ "error": error.to_string() }),
    }
}

/// The served side: the operations through the reader. The freshness step
/// stays out of the number: the store is caught up once, then every read
/// runs under `annotate_only`. The scan comes from the test-set the comparison tests
/// took before the clock started.
async fn served_answer(store: &TestStore, request: &Request) -> Value {
    let policy = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);
    let mut answer = served_value(store, request, policy).await;
    strip_additive(&mut answer);
    answer
}

/// One catch-up so the served side has a projection, then the scan both
/// sides read from.
async fn prepare(store: &TestStore) -> (Vec<FileScan>, f64, f64) {
    let layout = store.layout();
    let started = Instant::now();
    crate::cache::catch_up_state(&layout).await.unwrap();
    let rebuild_ms = timing::elapsed_ms(started);
    // The steady-state pass, which every read under `catch_up` pays.
    let started = Instant::now();
    crate::cache::catch_up_state(&layout).await.unwrap();
    let catch_up_ms = timing::elapsed_ms(started);
    let scans = provenance_scanner::scan_path(&store.root).unwrap();
    crate::test_probes::set_test_scan(Some(scans.clone()));
    (scans, rebuild_ms, catch_up_ms)
}

async fn assert_agreement(store: TestStore) {
    let (scans, _, _) = prepare(&store).await;
    let requests = requests::for_store(&store);
    assert!(
        requests.len() >= 8,
        "{}: the request set must cover the operations",
        store.name
    );
    for request in &requests {
        let baseline = baseline_answer(&store, request, &scans);
        let served = served_answer(&store, request).await;
        assert_eq!(
            baseline,
            served,
            "{} {} over {} must answer as the baseline does",
            request.operation(),
            request.describe(),
            store.name
        );
    }
    crate::test_probes::set_test_scan(None);
}

/// Times every case the comparison tests runs over one store and prints the rows.
async fn print_timings(store: TestStore) {
    let (scans, rebuild_ms, catch_up_ms) = prepare(&store).await;
    let started = Instant::now();
    provenance_scanner::scan_path(&store.root).unwrap();
    let scan_ms = timing::elapsed_ms(started);
    let mut rows = Vec::new();
    for request in &requests::for_store(&store) {
        baseline_answer(&store, request, &scans);
        served_answer(&store, request).await;
        let mut baseline_samples = Vec::new();
        let mut served_samples = Vec::new();
        for _ in 0..timing::RUNS {
            let started = Instant::now();
            baseline_answer(&store, request, &scans);
            baseline_samples.push(timing::elapsed_ms(started));
            let started = Instant::now();
            served_answer(&store, request).await;
            served_samples.push(timing::elapsed_ms(started));
        }
        rows.push(timing::Row {
            operation: request.operation(),
            request: request.describe(),
            baseline_ms: timing::median(&mut baseline_samples),
            served_ms: timing::median(&mut served_samples),
        });
    }
    crate::test_probes::set_test_scan(None);
    timing::print_rows(store.name, &rows, scan_ms, rebuild_ms, catch_up_ms);
}

#[tokio::test]
async fn served_answers_match_the_baseline_over_the_seeded_query_store() {
    assert_agreement(test_stores::seeded_queries()).await;
}

#[tokio::test]
async fn served_answers_match_the_baseline_over_the_cache_fixtures() {
    for store in test_stores::cache_fixtures() {
        assert_agreement(store).await;
    }
}

#[tokio::test]
async fn served_answers_match_the_baseline_over_the_repository_state() {
    assert_agreement(test_stores::repository_state()).await;
}

/// The timing comparison report over every store. Run it by hand:
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
