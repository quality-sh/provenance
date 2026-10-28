//! The host registers its location only when it can answer discovery.
#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_transport::local_host::discover;
use review_host_support::{repository, review_command, spawn_command, waits};
use serde_json::Value;
use std::{path::Path, process::Command, time::Duration};

fn create_requirement(root: &Path) {
    let output = Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "req_start",
            "create",
            "--type",
            "requirement",
            "--repo",
            root.to_str().unwrap(),
            "--statement",
            "The host answers before it is discoverable.",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn the_host_announces_itself_only_when_it_answers() {
    let repo = repository();
    create_requirement(repo.path());
    let layout = ProvenanceLayout::new(repo.path().to_str().unwrap());
    let registry = repo
        .path()
        .join(".provenance/cache/local-hosts/default.json");
    let mut starting = None;
    // Startup reads the graph under the publication guard, so the host cannot answer while
    // this test holds the guard.
    waits::publish(&layout, || {
        let mut host = spawn_command(review_command(repo.path()));
        std::thread::sleep(Duration::from_millis(1500));
        assert!(
            !host.has_started(),
            "the host must not report readiness while it cannot answer"
        );
        assert!(
            !registry.exists(),
            "the registry must not name a host that does not answer yet"
        );
        assert!(discover(repo.path(), "default").unwrap().is_none());
        starting = Some(host);
        Ok(())
    })
    .unwrap();
    let host = starting.unwrap().started(Duration::from_secs(15));

    assert!(registry.exists(), "the registry names the serving host");
    let output = host
        .cli()
        .args([
            "req_start",
            "--review-link",
            "--no-open",
            "--repo",
            repo.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let link: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(link["review_url"].as_str().unwrap().starts_with(&format!(
        "{}/?root=req_start#launch=",
        host.config["endpoint"].as_str().unwrap()
    )));
}
