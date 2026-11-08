use crate::support::{cli_stale, commit as commit_all, fixture, write, Fixture, Goldens};
use provenance_core::protocol::{StaleQuery, SDK_PROTOCOL_VERSION};
use provenance_store::operations::queries;
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::repository::Repository;
use serde_json::Value;

/// One stale request: a name for its recorded answer and its parameters.
struct Request {
    name: &'static str,
    base: &'static str,
    head: Option<&'static str>,
    rules: &'static [&'static str],
    limit: usize,
}

const REQUESTS: [Request; 6] = [
    Request {
        name: "range",
        base: "BASE",
        head: Some("HEAD"),
        rules: &[],
        limit: 50,
    },
    Request {
        name: "range_one_rule",
        base: "BASE",
        head: Some("HEAD"),
        rules: &["rule_touched", "rule_crossed"],
        limit: 50,
    },
    Request {
        name: "range_first_page",
        base: "BASE",
        head: Some("HEAD"),
        rules: &[],
        limit: 2,
    },
    Request {
        name: "range_reversed",
        base: "HEAD",
        head: Some("BASE"),
        rules: &[],
        limit: 50,
    },
    Request {
        name: "range_empty",
        base: "HEAD",
        head: Some("HEAD"),
        rules: &[],
        limit: 50,
    },
    Request {
        name: "range_default_head",
        base: "BASE",
        head: None,
        rules: &[],
        limit: 50,
    },
];

fn commit(fixture: &Fixture, name: &str) -> String {
    match name {
        "BASE" => fixture.base.clone(),
        "HEAD" => fixture.head.clone(),
        other => panic!("unknown commit name {other}"),
    }
}

fn cli_answer(fixture: &Fixture, request: &Request) -> Value {
    let base = commit(fixture, request.base);
    let head = request.head.map(|head| commit(fixture, head));
    let limit = request.limit.to_string();
    let mut query = vec![("base", base.as_str()), ("limit", limit.as_str())];
    if let Some(head) = &head {
        query.push(("head", head.as_str()));
    }
    let rules = request.rules.join(",");
    if !rules.is_empty() {
        query.push(("rules", rules.as_str()));
    }
    let (code, stdout, stderr) = cli_stale(fixture.path(), &query);
    assert_eq!(code, 0, "{} failed: {stderr}", request.name);
    fixture.normalized(&stdout["data"])
}

async fn store_answer(fixture: &Fixture, repository: &Repository, request: &Request) -> Value {
    let answer = queries::stale(
        repository,
        ReadPolicy::default(),
        StaleQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            base: commit(fixture, request.base),
            head: request.head.map(|head| commit(fixture, head)),
            rules: request.rules.iter().map(ToString::to_string).collect(),
            limit: request.limit,
        },
    )
    .await
    .unwrap();
    fixture.normalized(&serde_json::to_value(answer.result).unwrap())
}

/// Each request from a new CLI process gives its recorded answer.
#[test]
fn each_cli_request_gives_the_recorded_answer() {
    let fixture = fixture();
    let mut goldens = Goldens::default();
    for request in &REQUESTS {
        goldens.check(
            &format!("cli_{}", request.name),
            &cli_answer(&fixture, request),
        );
    }
    goldens.finish();
}

/// One process that answers the requests many times, in a mixed order,
/// gives the recorded answer each time. After a new commit, its answer
/// for the new range is the answer of a new process.
#[tokio::test(flavor = "multi_thread")]
async fn repeated_requests_in_one_process_give_the_recorded_answers() {
    let mut fixture = fixture();
    let repository = Repository::open(fixture.path(), "default").unwrap();
    let mut goldens = Goldens::default();
    for index in [0, 0, 3, 0, 1, 2, 4, 5, 0, 3, 3, 0] {
        let request = &REQUESTS[index];
        let answer = store_answer(&fixture, &repository, request).await;
        goldens.check(&format!("store_{}", request.name), &answer);
    }
    goldens.finish();

    write(
        fixture.path(),
        "src/kept.rs",
        "#[rule(\"rule_kept\")]\nfn kept() -> u32 {\n    5\n}\n",
    );
    fixture.head = commit_all(fixture.path(), "later head");
    let in_process = store_answer(&fixture, &repository, &REQUESTS[0]).await;
    let new_process = cli_answer(&fixture, &REQUESTS[0]);
    assert_eq!(in_process["sites"], new_process["items"]);
    assert_eq!(in_process["summary"], new_process["summary"]);
    assert!(in_process["sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["subject_id"] == "rule_kept"));
}
