//! A verification run row stays inside the byte budget of the resource reads
//! of the runs. The start and the completion refuse a larger row before they
//! write, and each read of the runs, the evidence read of each Rule included,
//! gives a row inside that budget.
#![cfg(feature = "test-fixture")]
use crate::support;
use provenance_core::{ScopeId, VerificationRun, VerificationRunStatus};
use provenance_macros::verifies;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};

/// The bytes of one stored row that the page read and the member read of
/// the runs give.
const BUDGET: usize = 1_048_576 - 16_384;

/// The byte budget of one page of a resource read.
const PAGE_BYTES: usize = 1_048_576;

/// A repository with the Rules `rule_shared` and `rule_shared_other`, and
/// the file that a run cites.
fn repository() -> Repository {
    let repo = Repository::new("The evidence is readable.");
    repo.seed(
        ScopeId::new("default").unwrap(),
        "The other evidence is readable.",
        "_other",
    );
    std::fs::write(repo.dir.path().join("check.rs"), "fn check() {}\n").unwrap();
    repo
}

async fn begin(host: &StatementHost, key: &str, symbol: &str) -> (u16, Value) {
    let start = json!({"data": {
        "rule": "rule_shared", "key": key, "method": "examples",
        "declared_by": "fixture", "file": "check.rs", "symbol": symbol
    }});
    let path = "/verification-runs/begin-verification";
    call(host, "POST", path, Some(start)).await
}

async fn fail(host: &StatementHost, run: &Value, error: usize) -> (u16, Value) {
    let id = run["data"]["id"].as_str().unwrap();
    let path = format!("/verification-runs/{id}/complete-verification");
    let body = json!({"data": {"status": "failed", "error": "x".repeat(error)}});
    call(host, "POST", &path, Some(body)).await
}

/// The length of the error text that makes the row of `run`, after a failed
/// completion, `row` bytes long.
fn error_for_row(run: &Value, row: usize) -> usize {
    let mut run: VerificationRun = serde_json::from_value(run["data"].clone()).unwrap();
    run.status = VerificationRunStatus::Failed;
    run.completed_at = Some(run.started_at);
    run.error = Some(String::new());
    row - serde_json::to_vec(&run).unwrap().len()
}

/// The status of the evidence read of each Rule, of the page read, and of
/// the member read of `run`.
async fn reads(host: &StatementHost, run: &str) -> Vec<(String, u16)> {
    let mut statuses = Vec::new();
    for path in [
        "/rules/rule_shared/evidence".to_owned(),
        "/rules/rule_shared_other/evidence".to_owned(),
        "/verification-runs".to_owned(),
        format!("/verification-runs/{run}"),
    ] {
        let (status, _) = call(host, "GET", &path, None).await;
        statuses.push((path, status));
    }
    statuses
}

fn all_ok(run: &str) -> Vec<(String, u16)> {
    [
        "/rules/rule_shared/evidence".to_owned(),
        "/rules/rule_shared_other/evidence".to_owned(),
        "/verification-runs".to_owned(),
        format!("/verification-runs/{run}"),
    ]
    .into_iter()
    .map(|path| (path, 200))
    .collect()
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn each_read_of_the_runs_gives_a_run_row_at_the_budget() {
    let repo = repository();
    let host = host(&repo, true);
    let (_, run) = begin(&host, "check", "check").await;
    let error = error_for_row(&run, BUDGET);

    let (status, completed) = fail(&host, &run, error).await;
    let id = completed["data"]["id"].as_str().unwrap();
    let (_, evidence) = call(&host, "GET", "/rules/rule_shared/evidence", None).await;

    assert_eq!(status, 200, "{}", completed["error"]);
    assert_eq!(reads(&host, id).await, all_ok(id));
    let runs = &evidence["data"]["verification_runs"];
    assert_eq!(runs[0]["error"].as_str().map(str::len), Some(error));
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn a_completion_with_a_row_over_the_budget_is_refused_and_the_run_stays_running() {
    let repo = repository();
    let host = host(&repo, true);
    let (_, run) = begin(&host, "check", "check").await;
    let id = run["data"]["id"].as_str().unwrap();

    let (status, refusal) = fail(&host, &run, error_for_row(&run, BUDGET + 1)).await;
    let (_, member) = call(&host, "GET", &format!("/verification-runs/{id}"), None).await;

    assert_eq!(
        (status, &refusal["error"]["kind"]),
        (400, &json!("record_too_large"))
    );
    assert_eq!(reads(&host, id).await, all_ok(id));
    assert_eq!(member["data"], run["data"]);
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn a_start_with_a_row_over_the_budget_writes_no_run_and_no_binding() {
    let repo = repository();
    let host = host(&repo, true);
    let (_, probe) = begin(&host, "probe", "s").await;
    let probe_row = serde_json::to_vec(&probe["data"]).unwrap().len();
    let symbol = "s".repeat(BUDGET + 2 - probe_row);

    let (status, refusal) = begin(&host, "check", &symbol).await;
    let (_, runs) = call(&host, "GET", "/verification-runs", None).await;
    let (_, bindings) = call(&host, "GET", "/verification-bindings", None).await;

    assert_eq!(
        (status, &refusal["error"]["kind"]),
        (400, &json!("record_too_large"))
    );
    assert_eq!(runs["data"]["items"], json!([probe["data"]]));
    let keys: Vec<&Value> = bindings["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|binding| &binding["key"])
        .collect();
    assert_eq!(keys, [&json!("probe")]);
}

/// The evidence read gives the newest runs of the Rule that fit in one page
/// of the runs, and flags the runs that it leaves out.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn the_runs_of_the_evidence_read_stay_inside_one_page() {
    let repo = repository();
    let host = host(&repo, true);
    let row = 64_000;
    let mut ids = Vec::new();
    for _ in 0..=PAGE_BYTES / row {
        let (_, run) = begin(&host, "check", "check").await;
        let (status, completed) = fail(&host, &run, error_for_row(&run, row)).await;
        assert_eq!(status, 200, "{}", completed["error"]);
        ids.push(completed["data"]["id"].as_str().unwrap().to_owned());
    }

    let (status, evidence) = call(&host, "GET", "/rules/rule_shared/evidence", None).await;

    assert_eq!(status, 200, "{}", evidence["error"]);
    let given: Vec<&str> = evidence["data"]["verification_runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["id"].as_str().unwrap())
        .collect();
    let newest: Vec<&str> = ids
        .iter()
        .rev()
        .take(PAGE_BYTES / row)
        .map(String::as_str)
        .collect();
    assert_eq!(given, newest);
    assert_eq!(evidence["data"]["verification_runs_has_more"], json!(true));
    assert_eq!(
        evidence["data"]["latest_verification_run"]["id"],
        json!(ids.last())
    );
}
