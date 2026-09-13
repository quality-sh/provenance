use provenance_transport::fixture::records::Repository;
use serde_json::{json, Value};

pub fn writable_host(repo: &Repository) -> provenance_transport::StatementHost {
    use provenance_transport::fixture::{FixtureAccess, Target};
    provenance_transport::StatementHost::with_fixture_access(
        FixtureAccess::new(
            vec![Target {
                id: "selected".into(),
                root: repo.dir.path().into(),
            }],
            vec![("selected".into(), "default".into())],
            "fixture-secret",
            "fixture.test",
        )
        .unwrap()
        .allow_writes(),
    )
}
pub fn scoped(request: &Value) -> Value {
    json!({"context":{"repository":"selected","scope":"default"},"request":request})
}
