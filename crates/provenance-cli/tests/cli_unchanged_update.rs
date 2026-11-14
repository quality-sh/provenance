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
use support::review_paths::{create_requirement, init, json_output, provenance};

/// The command that sets the description of `req_a` to one text.
fn update_description(repo: &str) -> assert_cmd::Command {
    let read = json_output(provenance().args(["req_a", "get", "--repo", repo, "--format", "json"]));
    let etag = read["edit"]["etag"].as_str().unwrap().to_owned();
    let mut command = provenance();
    command.args([
        "req_a",
        "update",
        "--repo",
        repo,
        "--if-match",
        &etag,
        "--description",
        "The stored description.",
    ]);
    command
}

#[test]
#[verifies("rule_empty_write_set_writes_no_data", examples)]
fn an_update_to_the_stored_content_writes_nothing() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    update_description(&repo).assert().success();
    let cache = Path::new(&repo).join(".provenance/cache");
    let transactions = cache.join("import-transactions");
    std::fs::create_dir_all(&transactions).unwrap();
    std::fs::set_permissions(&transactions, std::fs::Permissions::from_mode(0o500)).unwrap();

    let updated = update_description(&repo).output().unwrap();

    std::fs::set_permissions(&transactions, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        updated.status.success(),
        "{}",
        String::from_utf8_lossy(&updated.stderr)
    );
    assert!(!cache.join("import-publication.json").exists());
}
