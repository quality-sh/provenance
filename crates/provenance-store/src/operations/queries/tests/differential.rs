//! Runs each served operation and its oracle copy over the same store and
//! asserts the answers agree, then times both sides and prints the rows.
//!
//! `cargo test -p provenance-store differential -- --nocapture` shows the
//! rows; `PROVENANCE_AB_GATE=1` makes the ceilings assert (timing.rs).

pub mod corpus;
pub mod requests;
mod timing;

use super::oracle;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader;
use corpus::Corpus;
use provenance_scanner::FileScan;
use requests::Request;
use serde_json::{json, Value};
use std::time::Instant;

/// Fields the served side adds beside the answer; the oracle has none.
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

fn oracle_answer(corpus: &Corpus, request: &Request, scans: &[FileScan]) -> Value {
    let store = corpus.store();
    let (repo, scope) = (corpus.root.as_path(), &corpus.scope);
    match request.clone() {
        Request::Get(query) => settle(oracle::records::get(&store, scope, query)),
        Request::Search(query) => settle(oracle::records::search(&store, scope, query)),
        Request::Neighbors(query) => settle(oracle::walk::neighbors(&store, scope, query)),
        Request::Trace(query) => settle(oracle::walk::trace(&store, scope, query)),
        Request::Impact(query) => settle(oracle::impact::impact(repo, &store, scope, query, scans)),
        Request::Evidence(query) => settle(oracle::evidence::evidence(repo, &store, scope, query)),
        Request::Stale(query) => settle(oracle::stale::stale(repo, scope, query)),
        Request::ResolveSymbol(query) => {
            settle(oracle::symbols::resolve(repo, &store, scope, query, scans))
        }
    }
}

/// One request through the reader under the given policy, as a value; a
/// refusal becomes `{"error": ..}` so both sides compare the same way.
pub async fn served_value(corpus: &Corpus, request: &Request, policy: ReadPolicy) -> Value {
    use super::super::{evidence, impact, records, stale, symbols, walk};
    let scope = corpus.scope.clone();
    let request = request.clone();
    let served = reader::answer(&corpus.root, &corpus.scope, policy, move |ctx| {
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

/// The served side: the executors through the reader. The freshness step
/// stays out of the number: the corpus is caught up once, then every read
/// runs under `annotate_only`. The scan comes from the preset the harness
/// took before the clock started.
async fn served_answer(corpus: &Corpus, request: &Request) -> Value {
    let policy = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);
    let mut answer = served_value(corpus, request, policy).await;
    strip_additive(&mut answer);
    answer
}

async fn run_corpus(corpus: Corpus) {
    let layout = corpus.layout();
    let started = Instant::now();
    crate::cache::catch_up_state(&layout).await.unwrap();
    let rebuild_ms = timing::elapsed_ms(started);
    // The steady-state pass, which every read under `catch_up` pays.
    let started = Instant::now();
    crate::cache::catch_up_state(&layout).await.unwrap();
    let catch_up_ms = timing::elapsed_ms(started);

    let started = Instant::now();
    let scans = provenance_scanner::scan_path(&corpus.root).unwrap();
    let scan_ms = timing::elapsed_ms(started);
    crate::test_probes::set_preset_scan(Some(scans.clone()));

    let requests = requests::for_corpus(&corpus);
    assert!(
        requests.len() >= 8,
        "{}: the request set must cover the operations",
        corpus.name
    );
    for request in &requests {
        let oracle = oracle_answer(&corpus, request, &scans);
        let served = served_answer(&corpus, request).await;
        assert_eq!(
            oracle,
            served,
            "{} {} over {} must answer as the oracle does",
            request.operation(),
            request.describe(),
            corpus.name
        );
    }

    let mut rows = Vec::new();
    let mut timed: Vec<&'static str> = Vec::new();
    for request in &requests {
        if timed.contains(&request.operation()) {
            continue;
        }
        timed.push(request.operation());
        oracle_answer(&corpus, request, &scans);
        served_answer(&corpus, request).await;
        let mut oracle_samples = Vec::new();
        let mut served_samples = Vec::new();
        for _ in 0..timing::runs() {
            let started = Instant::now();
            oracle_answer(&corpus, request, &scans);
            oracle_samples.push(timing::elapsed_ms(started));
            let started = Instant::now();
            served_answer(&corpus, request).await;
            served_samples.push(timing::elapsed_ms(started));
        }
        rows.push(timing::Row {
            operation: request.operation(),
            request: request.describe(),
            oracle_ms: timing::median(&mut oracle_samples),
            served_ms: timing::median(&mut served_samples),
        });
    }
    crate::test_probes::set_preset_scan(None);
    timing::print_rows(corpus.name, &rows, scan_ms, rebuild_ms, catch_up_ms);
    timing::check_ceilings(corpus.name, &rows, scan_ms, catch_up_ms);
}

#[tokio::test]
async fn served_answers_match_the_oracle_over_the_seeded_query_store() {
    run_corpus(corpus::seeded_queries()).await;
}

#[tokio::test]
async fn served_answers_match_the_oracle_over_the_cache_fixtures() {
    for corpus in corpus::cache_fixtures() {
        run_corpus(corpus).await;
    }
}

#[tokio::test]
async fn served_answers_match_the_oracle_over_the_repository_state() {
    run_corpus(corpus::repository_state()).await;
}
