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

#[test]
// Implementation aid: a native connection must present one valid key and no browser credential.
fn native_subscription_refuses_ambiguous_and_browser_credentials() {
    let repo = repository();
    let host = start(repo.path());
    let key = std::fs::read_to_string(host.launch_key_path()).unwrap();
    let header = format!("X-Provenance-Launch-Key: {key}\r\n");
    for headers in [
        "X-Provenance-Launch-Key: wrong\r\n".to_owned(),
        format!("{header}{header}"),
        format!("{header}Cookie: arbitrary=value\r\n"),
        format!("{header}Sec-Fetch-Site: same-origin\r\n"),
        format!(
            "{header}Origin: {}\r\n",
            host.config["endpoint"].as_str().unwrap()
        ),
    ] {
        let (status, _) = upgrade(&host, "/review-events", &headers);
        assert!(
            matches!(status, 401 | 403),
            "native credential was accepted: {status}"
        );
    }
    let (status, _) = upgrade(&host, "/review-events?key=unused", &header);
    assert_eq!(status, 403);
}
