pub use provenance_transport::fixture::records::Repository;
use serde_json::{json, Value};
pub fn get_call(target: &str, scope: &str) -> Value {
    json!({"context":{"repository":target,"scope":scope},"request":{"node_type":"rule","id":"rule_shared"}})
}

pub fn host(
    repositories: &[(&str, &Repository)],
    allowed: &[&str],
) -> provenance_transport::StatementHost {
    use provenance_transport::fixture::{FixtureAccess, Target};
    let targets = repositories
        .iter()
        .map(|(id, repo)| Target {
            id: (*id).to_owned(),
            root: repo.dir.path().to_path_buf(),
        })
        .collect();
    let grants = allowed
        .iter()
        .flat_map(|target| {
            ["default", "other"].map(|scope| ((*target).to_owned(), scope.to_owned()))
        })
        .collect();
    provenance_transport::StatementHost::with_fixture_access(
        FixtureAccess::new(targets, grants, "fixture-secret", "fixture.test").unwrap(),
    )
}

pub async fn call(
    host: &provenance_transport::StatementHost,
    operation: &str,
    body: Value,
) -> (u16, Value) {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let response = host
        .router()
        .oneshot(
            Request::post(format!("/v7/operations/{operation}"))
                .header("host", "fixture.test")
                .header("authorization", "Bearer fixture-secret")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (status, value)
}
