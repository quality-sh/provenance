#[path = "query_support/fixtures.rs"]
mod fixtures;

use provenance_macros::verifies;
use serde_json::json;

#[test]
#[verifies("rule_completed_read_leaves_no_wal_files", examples)]
fn completed_cli_reads_leave_no_wal_or_shm_files() {
    for round in 0..40 {
        let repo = fixtures::init_repo();
        let path = repo.path().to_str().unwrap();
        let cache = repo.path().join(".provenance/cache");
        for policy in ["catch_up", "annotate_only"] {
            fixtures::provenance()
                .args(["sdk", "get", "--repo", path, "--freshness", policy])
                .write_stdin(json!({"node_type": "requirement", "id": "req_missing"}).to_string())
                .assert()
                .success();
            assert!(
                !cache.join("provenance.db-wal").exists(),
                "read {round} with {policy} left the -wal file"
            );
            assert!(
                !cache.join("provenance.db-shm").exists(),
                "read {round} with {policy} left the -shm file"
            );
        }
    }
}
