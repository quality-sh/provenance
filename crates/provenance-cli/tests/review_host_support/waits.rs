//! Deadlines for real host test operations.
use provenance_store::{publication::PublicationGuard, repository::Repository};
use std::{
    process::{Command, Output},
    time::Duration,
};

pub const LIMIT: Duration = Duration::from_secs(60);

pub async fn guard(repository: &Repository) -> PublicationGuard {
    tokio::time::timeout(
        LIMIT,
        provenance_store::publication::publication_guard(repository),
    )
    .await
    .expect("publication lock acquisition exceeded 60 seconds")
    .unwrap()
}

pub fn output(command: &mut Command) -> Output {
    super::bounded_process::output(command, LIMIT)
}

/// Opens the fixture repository at `root` for the `default` scope.
pub fn repository(root: &std::path::Path) -> Repository {
    Repository::open(root, "default").unwrap()
}

/// Runs a publication with a deadline on lock acquisition.
pub fn publish(
    repository: &Repository,
    operation: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let guard = runtime.block_on(guard(repository));
    let result = operation();
    drop(guard);
    result
}
