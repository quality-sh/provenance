use provenance_macros::verifies;
use provenance_transport::{
    fixture::local_host::LocalHostFixture,
    local_host::{discover, LocalHostRegistration},
};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn registry(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".provenance/cache/local-hosts/default.json")
}

#[cfg(unix)]
#[test]
#[verifies("rule_local_host_registry_owner_only", examples)]
fn publication_repairs_permissive_registry_modes() {
    let repository = tempfile::tempdir().unwrap();
    let path = registry(repository.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::set_permissions(
        path.parent().unwrap(),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    std::fs::write(&path, b"{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();

    let _registration = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1234",
        "local",
    )
    .unwrap();

    assert_eq!(
        std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o077,
        0
    );
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o077,
        0
    );
}

#[test]
#[verifies("rule_single_local_host_per_repository_scope", examples)]
fn publication_refuses_a_live_slot_owner() {
    let repository = tempfile::tempdir().unwrap();
    let first = LocalHostFixture::start(repository.path(), "default", "local");

    let error = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1235",
        "local",
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("already has a running local host"));
    assert_eq!(
        discover(repository.path(), "default")
            .unwrap()
            .unwrap()
            .endpoint()
            .as_str(),
        format!("{}/", first.endpoint)
    );
}

#[test]
#[verifies("rule_single_local_host_per_repository_scope", examples)]
fn publication_reclaims_a_slot_after_the_previous_host_stops() {
    let repository = tempfile::tempdir().unwrap();
    let mut first = LocalHostFixture::start(repository.path(), "default", "old");
    first.stop_listener();

    let replacement = LocalHostFixture::start(repository.path(), "default", "new");

    assert_eq!(
        discover(repository.path(), "default")
            .unwrap()
            .unwrap()
            .endpoint()
            .as_str(),
        format!("{}/", replacement.endpoint)
    );
}

#[test]
#[verifies("rule_local_host_stop_removes_own_registry", examples)]
fn registration_removes_its_record_on_drop() {
    let repository = tempfile::tempdir().unwrap();
    let first = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1234",
        "local",
    )
    .unwrap();
    let second = LocalHostRegistration::publish(
        repository.path(),
        "default",
        "http://127.0.0.1:1235",
        "local",
    )
    .unwrap();

    drop(first);

    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(registry(repository.path())).unwrap()).unwrap();
    assert_eq!(
        record["instanceNonce"],
        second.identity().instance_nonce.as_str()
    );
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn discovery_returns_a_matching_live_host() {
    let repository = tempfile::tempdir().unwrap();
    let host = LocalHostFixture::start(repository.path(), "default", "local");

    let discovered = discover(repository.path(), "default").unwrap().unwrap();

    assert_eq!(
        discovered.endpoint().as_str(),
        format!("{}/", host.endpoint)
    );
}

#[test]
#[verifies("rule_local_host_discovery_removes_stale_registry", examples)]
fn discovery_removes_stale_state() {
    let repository = tempfile::tempdir().unwrap();
    let registration =
        LocalHostRegistration::publish(repository.path(), "default", "http://127.0.0.1:1", "local")
            .unwrap();
    std::mem::forget(registration);

    discover(repository.path(), "default").unwrap();
    assert!(!registry(repository.path()).exists());
}

#[test]
#[verifies("rule_local_host_discovery_removes_stale_registry", examples)]
fn discovery_removes_an_identity_mismatch() {
    let repository = tempfile::tempdir().unwrap();
    let _host =
        LocalHostFixture::start_with_identity(repository.path(), "default", "local", |identity| {
            let mut response = serde_json::to_value(identity).unwrap();
            response["repositoryId"] = "other".into();
            response
        });

    assert!(discover(repository.path(), "default").unwrap().is_none());
    assert!(!registry(repository.path()).exists());
}
