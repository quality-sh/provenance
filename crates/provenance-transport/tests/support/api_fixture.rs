//! Fixture helpers shared by the api conformance tests.
use provenance_transport::fixture::{FixtureAccess, Target};
use provenance_transport::StatementHost;

pub use super::records::Repository;

/// One fully granted fixture principal bound to the repository.
pub fn access(repository: &Repository) -> FixtureAccess {
    FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
}

/// One read-only fixture host over the repository.
pub fn host(repository: &Repository) -> StatementHost {
    StatementHost::with_fixture_access(access(repository))
}

/// One fixture host that permits record writes.
#[allow(dead_code)]
pub fn write_host(repository: &Repository) -> StatementHost {
    StatementHost::with_fixture_access(access(repository).allow_writes())
}
