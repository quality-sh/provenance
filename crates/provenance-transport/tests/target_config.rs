#![cfg(feature = "test-fixture")]
use provenance_transport::fixture::{FixtureAccess, Target};

#[test]
fn fixture_configuration_rejects_duplicate_and_malformed_targets() {
    let directory = tempfile::tempdir().unwrap();
    let target = |id: &str| Target {
        id: id.into(),
        root: directory.path().to_path_buf(),
    };
    for targets in [
        vec![target("same"), target("same")],
        vec![target("")],
        vec![target("../root")],
        vec![target("bad\nname")],
        vec![Target {
            id: "relative".into(),
            root: "relative".into(),
        }],
    ] {
        assert!(FixtureAccess::new(targets, vec![], "token", "fixture.test").is_err());
    }
    assert!(FixtureAccess::new(
        vec![target("first")],
        vec![("missing".into(), "default".into())],
        "token",
        "fixture.test"
    )
    .is_err());
    assert!(FixtureAccess::new(vec![target("first")], vec![], "", "fixture.test").is_err());
}
