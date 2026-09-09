use provenance_http_client::{Error, HttpClient, PROTOCOL_VERSION};
use serde_json::{json, Value};

struct Fixture {
    client: HttpClient,
    data: Value,
}
impl Fixture {
    async fn connect() -> Self {
        let data: Value = serde_json::from_str(
            &std::env::var("PROVENANCE_RECORDS_FIXTURE").expect("records host fixture"),
        )
        .unwrap();
        let client = HttpClient::connect_with_bearer(
            data["url"].as_str().unwrap(),
            data["bearer"].as_str().unwrap(),
        )
        .await
        .unwrap();
        Self { client, data }
    }
    fn context(&self) -> Value {
        json!({"repository":self.data["targets"]["first"],"scope":"default"})
    }
    async fn raw(&self, operation: &str, call: &Value) -> (u16, Value) {
        let response = reqwest::Client::new()
            .post(format!(
                "{}/v{PROTOCOL_VERSION}/operations/{operation}",
                self.data["url"].as_str().unwrap()
            ))
            .bearer_auth(self.data["bearer"].as_str().unwrap())
            .json(call)
            .send()
            .await
            .unwrap();
        (response.status().as_u16(), response.json().await.unwrap())
    }
}

macro_rules! compare {
    ($fixture:expr, $method:ident, $call:expr) => {{
        let call = $call;
        let (status, expected) = $fixture
            .raw(stringify!($method).replace('_', "-").as_str(), &call)
            .await;
        assert_eq!(status, 200);
        let actual = $fixture
            .client
            .$method(&serde_json::from_value(call).unwrap())
            .await
            .unwrap();
        let actual = serde_json::to_value(actual).unwrap();
        assert_eq!(
            actual, expected,
            "all fields and the full stamp must survive"
        );
        actual
    }};
}
macro_rules! refuses {
    ($fixture:expr, $method:ident, $call:expr, $status:expr, $kind:expr) => {{
        let call = $call;
        let (status, expected) = $fixture
            .raw(stringify!($method).replace('_', "-").as_str(), &call)
            .await;
        assert_eq!(status, $status);
        match $fixture
            .client
            .$method(&serde_json::from_value(call).unwrap())
            .await
            .unwrap_err()
        {
            Error::Operation { status, failure } => {
                assert_eq!(status, $status);
                let actual = serde_json::to_value(failure).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(actual["error"]["kind"], $kind);
                actual
            }
            error => panic!("expected typed operation refusal, received {error}"),
        }
    }};
}

#[tokio::test]
#[ignore = "requires the isolated evidence host fixture"]
async fn evidence_null_cuts_and_git_diff_keep_every_field() {
    let fixture = Fixture::connect().await;
    let rule = &fixture.data["evidence"]["rule_id"];
    let base = &fixture.data["evidence"]["base"];
    let result = compare!(
        fixture,
        evidence,
        json!({"context":fixture.context(),"request":{"rule":rule}})
    );
    assert!(result["stale"].is_null());
    assert!(result["verification_runs"].as_array().unwrap().len() > 1);
    let cut = compare!(
        fixture,
        evidence,
        json!({"context":fixture.context(),"request":{"rule":rule,"limit":1}})
    );
    for field in [
        "implementation_bindings",
        "verification_bindings",
        "verification_runs",
        "reviews",
    ] {
        assert_eq!(cut[field].as_array().unwrap().len(), 1, "{field}");
        assert_eq!(cut[format!("{field}_has_more")], true);
    }
    assert_eq!(cut["has_more"], true);
    let based = compare!(
        fixture,
        evidence,
        json!({"context":fixture.context(),"request":{"rule":rule,"base":base}})
    );
    assert_eq!(&based["stale"]["base"], base);
    assert!(!based["stale"]["sites"].as_array().unwrap().is_empty());
    let no_git = compare!(
        fixture,
        evidence,
        json!({"context":{"repository":fixture.data["targets"]["second"],"scope":"default"},"request":{"rule":rule,"head":"unused-without-base"}})
    );
    assert!(no_git["stale"].is_null());
    assert!(no_git.get("latest_verification_run").is_none());
}

#[tokio::test]
#[ignore = "requires the isolated evidence host fixture"]
async fn impact_symbols_and_filtered_stale_are_concrete() {
    let fixture = Fixture::connect().await;
    let evidence = &fixture.data["evidence"];
    let impact = compare!(
        fixture,
        impact,
        json!({"context":fixture.context(),"request":{"id":evidence["rule_id"]}})
    );
    assert!(!impact["affected_rules"].as_array().unwrap().is_empty());
    assert!(impact["scan_cut"].is_boolean());
    let symbol = compare!(
        fixture,
        resolve_symbol,
        json!({"context":fixture.context(),"request":{"file":evidence["file"]}})
    );
    assert!(!symbol["rules"].as_array().unwrap().is_empty());
    assert!(symbol.get("symbol").is_none());
    let stale = compare!(
        fixture,
        stale,
        json!({"context":fixture.context(),"request":{"base":evidence["base"],"rules":[evidence["rule_id"]]}})
    );
    assert!(!stale["sites"].as_array().unwrap().is_empty());
}

#[tokio::test]
#[ignore = "requires the isolated evidence host fixture"]
async fn verification_lists_are_complete_filtered_and_scoped() {
    let fixture = Fixture::connect().await;
    for rule in [Value::Null, fixture.data["evidence"]["rule_id"].clone()] {
        let call = json!({"context":fixture.context(),"request":{"rule":rule}});
        let runs = compare!(fixture, verification_runs, call.clone());
        let bindings = compare!(fixture, verification_bindings, call);
        assert!(runs.as_array().unwrap().len() > 1);
        assert!(bindings.as_array().unwrap().len() > 1);
    }
    let other = json!({"context":{"repository":fixture.data["targets"]["first"],"scope":"other"},"request":{}});
    assert_eq!(
        compare!(fixture, verification_runs, other.clone()),
        json!([])
    );
    assert_eq!(compare!(fixture, verification_bindings, other), json!([]));
}

#[tokio::test]
#[ignore = "requires the isolated evidence host fixture"]
async fn file_and_git_refusals_are_typed() {
    let fixture = Fixture::connect().await;
    refuses!(
        fixture,
        resolve_symbol,
        json!({"context":fixture.context(),"request":{"file":"../outside.rs"}}),
        403,
        "file_access_denied"
    );
    refuses!(
        fixture,
        stale,
        json!({"context":fixture.context(),"request":{"base":"missing-revision"}}),
        409,
        "git_revision_not_found"
    );
    refuses!(
        fixture,
        evidence,
        json!({"context":{"repository":fixture.data["targets"]["second"],"scope":"default"},"request":{"rule":fixture.data["evidence"]["rule_id"],"base":"HEAD"}}),
        503,
        "git_unavailable"
    );
}
