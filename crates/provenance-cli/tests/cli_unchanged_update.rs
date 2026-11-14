//! An update to the stored content of a record writes nothing. Unix only:
//! the test takes the write permission from the folder of whole-set
//! transactions, so an update that makes a transaction fails.
#![cfg(unix)]

use provenance_macros::verifies;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, init, provenance};

fn update_description(repo: &str) {
    provenance()
        .args([
            "req_a",
            "update",
            "--repo",
            repo,
            "--description",
            "The stored description.",
        ])
        .assert()
        .success();
}

#[test]
#[verifies("rule_empty_write_set_writes_no_data", examples)]
fn an_update_to_the_stored_content_writes_nothing() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    update_description(&repo);
    let cache = Path::new(&repo).join(".provenance/cache");
    let transactions = cache.join("import-transactions");
    std::fs::create_dir_all(&transactions).unwrap();
    std::fs::set_permissions(&transactions, std::fs::Permissions::from_mode(0o500)).unwrap();

    let updated = provenance()
        .args([
            "req_a",
            "update",
            "--repo",
            &repo,
            "--description",
            "The stored description.",
        ])
        .output()
        .unwrap();

    std::fs::set_permissions(&transactions, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        updated.status.success(),
        "{}",
        String::from_utf8_lossy(&updated.stderr)
    );
    assert!(!cache.join("import-publication.json").exists());
}
