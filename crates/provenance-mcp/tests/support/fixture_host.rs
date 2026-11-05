//! A fixture host that grants every MCP tool and action.

use super::{check_port::FixtureCheckPort, records::Repository};
use provenance_porcelain::get::{Notify, NotifyPort, PortFuture, Registration};
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use std::sync::Arc;

/// A review host client that accepts each registration.
pub struct FixtureNotify;

impl NotifyPort for FixtureNotify {
    fn register<'a>(
        &'a self,
        _: &'a str,
        _: usize,
        notify: &'a Notify,
    ) -> PortFuture<'a, Registration> {
        Box::pin(async move {
            Ok(Registration {
                client: notify.client,
                thread: notify.thread.clone(),
                registered: true,
                accepted: true,
                removed: false,
            })
        })
    }
}

fn access(repository: &Repository) -> FixtureAccess {
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

/// A host that reads and writes the repository, checks it, and registers notify threads.
pub fn full(repository: &Repository) -> StatementHost {
    StatementHost::with_fixture_access(access(repository).allow_writes())
        .with_check_port(Arc::new(FixtureCheckPort))
        .with_notify_port(Arc::new(FixtureNotify))
}

/// A host that only reads the repository.
pub fn read_only(repository: &Repository) -> StatementHost {
    StatementHost::with_fixture_access(access(repository))
        .with_check_port(Arc::new(FixtureCheckPort))
}
