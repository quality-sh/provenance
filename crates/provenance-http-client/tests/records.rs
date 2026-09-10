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
    fn selected(&self, repository: &Value, scope: &str) -> Value {
        json!({"context":{"repository":repository,"scope":scope},"request":{"node_type":"rule","id":self.data["shared_rule"]}})
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
        let (status, expected) = $fixture.raw(stringify!($method), &call).await;
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
        let (status, expected) = $fixture.raw(stringify!($method), &call).await;
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
#[ignore = "run through node tools/operation-codegen/test-clients.mjs records"]
async fn metadata_and_all_eight_node_variants_keep_their_types() {
    let f = Fixture::connect().await;
    let info = compare!(
        f,
        info,
        json!({"context":{"repository":f.data["targets"]["first"]},"request":{}})
    );
    assert_eq!(info["repository"], f.data["targets"]["first"]);
    assert_eq!(info["protocol_version"], PROTOCOL_VERSION);
    let nodes = f.data["nodes"].as_object().unwrap();
    assert_eq!(nodes.len(), 8);
    for (node_type, id) in nodes {
        let record = compare!(
            f,
            get,
            json!({"context":f.context(),"request":{"node_type":node_type,"id":id}})
        );
        assert_eq!(record["found"], true);
        assert_eq!(record["node"]["node_type"], *node_type);
        assert_eq!(record["node"]["id"], *id);
        assert!(record["stamp"]["serial"].is_i64());
        assert!(record["stamp"]["instance_id"].is_string());
    }
}

#[tokio::test]
#[ignore = "run through node tools/operation-codegen/test-clients.mjs records"]
async fn selected_repository_scope_and_omissions_survive() {
    let f = Fixture::connect().await;
    for (repository, scope, expected) in [
        (
            &f.data["targets"]["first"],
            "default",
            &f.data["expected"]["first"],
        ),
        (
            &f.data["targets"]["second"],
            "default",
            &f.data["expected"]["second"],
        ),
        (
            &f.data["targets"]["first"],
            "other",
            &f.data["expected"]["other"],
        ),
    ] {
        let value = compare!(f, get, f.selected(repository, scope));
        assert_eq!(&value["node"]["statement"], expected);
    }
    let missing = compare!(
        f,
        get,
        json!({"context":f.context(),"request":{"node_type":"rule","id":"rule_missing"}})
    );
    assert_eq!(missing["found"], false);
    assert!(missing.get("node").is_none());
    let mut nullable = f.selected(&f.data["targets"]["first"], "default");
    nullable["context"]["freshness"] = Value::Null;
    let value = compare!(f, get, nullable);
    assert_eq!(value["node"]["statement"], f.data["expected"]["first"]);
}

#[tokio::test]
#[ignore = "run through node tools/operation-codegen/test-clients.mjs records"]
async fn all_bounded_methods_preserve_cuts_defaults_and_stamps() {
    let f = Fixture::connect().await;
    let search = compare!(
        f,
        search,
        json!({"context":f.context(),"request":{"text":"shared","limit":1}})
    );
    assert_eq!(search["has_more"], true);
    assert_eq!(search["nodes"].as_array().unwrap().len(), 1);
    let next = compare!(
        f,
        search,
        json!({"context":f.context(),
        "request":{"text":"shared","limit":1,"cursor":search["next_cursor"]}})
    );
    assert_ne!(search["nodes"][0]["id"], next["nodes"][0]["id"]);
    refuses!(
        f,
        search,
        json!({"context":f.context(),
        "request":{"text":"other","limit":1,"cursor":search["next_cursor"]}}),
        409,
        "cursor_invalid"
    );
    let mut cursor = Value::Null;
    let mut ids = std::collections::BTreeSet::new();
    loop {
        let call =
            json!({"context":f.context(),"request":{"id":"req_shared","limit":2,"cursor":cursor}});
        let (status, expected) = f.raw("read-document", &call).await;
        assert_eq!(status, 200, "{expected}");
        let actual = f
            .client
            .read_document(&serde_json::from_value(call).unwrap())
            .await
            .unwrap();
        let actual = serde_json::to_value(actual).unwrap();
        assert_eq!(actual, expected);
        for entry in actual["entries"].as_array().unwrap() {
            assert!(ids.insert(format!(
                "{}:{}:{}",
                entry["kind"], entry["node"]["node_type"], entry["node"]["id"]
            )));
        }
        cursor = actual["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert!(ids.len() >= 6);
    let defaults = compare!(
        f,
        search,
        json!({"context":f.context(),"request":{"text":"shared"}})
    );
    assert_eq!(defaults["limit"], 50);
    let call = json!({"context":f.context(),"request":{"id":f.data["nodes"]["requirement"],"node_type":"requirement","limit":1}});
    let neighbors = compare!(f, neighbors, call.clone());
    assert_eq!(neighbors["has_more"], true);
    assert_eq!(neighbors["neighbors"].as_array().unwrap().len(), 1);
    let trace = compare!(f, trace, call);
    assert_eq!(trace["has_more"], true);
    assert_eq!(trace["nodes"].as_array().unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "run through node tools/operation-codegen/test-clients.mjs records"]
async fn declared_failures_keep_their_fields_and_operation_identity() {
    let f = Fixture::connect().await;
    let zero = json!({"context":f.context(),"request":{"text":"shared","limit":0}});
    assert!(
        serde_json::from_value::<provenance_http_client::types::SearchRequestInput>(zero.clone())
            .is_err()
    );
    let (status, refused) = f.raw("search", &zero).await;
    assert_eq!(status, 400);
    assert_eq!(refused["error"]["kind"], "invalid_input");
    let typed: provenance_http_client::types::SearchFailureOutput =
        serde_json::from_value(refused.clone()).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), refused);
    refuses!(
        f,
        search,
        json!({"context":f.context(),"request":{"text":"shared","limit":201}}),
        400,
        "invalid_input"
    );
    refuses!(
        f,
        get,
        f.selected(&json!("unknown"), "default"),
        404,
        "unknown_target"
    );
    refuses!(
        f,
        get,
        f.selected(&f.data["denied_target"], "default"),
        403,
        "access_denied"
    );
    refuses!(
        f,
        get,
        f.selected(&f.data["targets"]["first"], "missing"),
        404,
        "unknown_scope"
    );
    let mut stale = f.selected(&f.data["stale_target"], "default");
    stale["context"]["freshness"] = json!("refuse_stale");
    let refusal = refuses!(f, get, stale, 409, "stale");
    assert!(refusal["error"]["serial"].is_i64());
    assert!(refusal["error"]["digest"].is_string());
    assert!(refusal["error"]["instance_id"].is_string());
    assert!(!refusal["error"]["moved"].as_array().unwrap().is_empty());
    let mut empty = f.selected(&f.data["unmaterialized_target"], "default");
    empty["context"]["freshness"] = json!("annotate_only");
    refuses!(f, get, empty, 409, "no_projection");
}
