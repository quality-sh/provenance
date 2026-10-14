#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{events::upgrade, repository, start};

#[test]
// Implementation aid: native credentials work without relaxing browser Origin checks.
fn native_key_upgrades_but_cross_origin_key_is_refused() {
    let repo = repository();
    let host = start(repo.path());
    let key = std::fs::read_to_string(host.launch_key_path()).unwrap();
    let header = format!("X-Provenance-Launch-Key: {key}\r\n");
    let (status, _) = upgrade(&host, "/review-events", &header);
    assert_eq!(status, 101);
    let (status, _) = upgrade(
        &host,
        "/review-events",
        &format!("{header}Origin: https://example.com\r\n"),
    );
    assert_eq!(status, 403);
}
