#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{events::{cookie, upgrade, Socket}, repository, start};
use std::time::Duration;

#[test]
// Implementation aid: event upgrades must enforce the existing local session policy.
fn event_upgrade_requires_one_cookie_origin_and_destination() {
    let repo = repository(); let host = start(repo.path());
    let mut valid = Socket::connect(&host); assert_eq!(valid.frame()["type"], "ready");
    let cookie = cookie(&host);
    let origin = host.config["endpoint"].as_str().unwrap();
    for headers in [
        format!("Origin: {origin}\r\n"),
        format!("Authorization: Bearer {}\r\nOrigin: {origin}\r\n", host.bearer()),
        format!("Cookie: {cookie}; {cookie}\r\nOrigin: {origin}\r\n"),
        format!("Cookie: {cookie}\r\n"),
        format!("Cookie: {cookie}\r\nOrigin: {origin}\r\nOrigin: {origin}\r\n"),
        format!("Cookie: {cookie}\r\nOrigin: http://foreign.test\r\n"),
        format!("Cookie: {cookie}\r\nOrigin: {origin}\r\nHost: {}\r\n", host.address()),
        format!("Cookie: {cookie}\r\nOrigin: {origin}\r\nSec-Fetch-Site: cross-site\r\n"),
        format!("Cookie: {cookie}\r\nOrigin: {origin}\r\nSec-Fetch-Site: same-origin\r\nSec-Fetch-Site: same-origin\r\n"),
    ] {
        let (status, _) = upgrade(&host, "/review-events", &headers);
        assert!(matches!(status, 400 | 401 | 403), "refusal before upgrade, got {status}");
    }
    let headers = format!("Cookie: {cookie}\r\nOrigin: {origin}\r\n");
    for path in ["/review-events?token=secret", "/review-events?bearer=secret", "/review-events?launch=secret"] {
        assert_eq!(upgrade(&host, path, &headers).0, 403);
    }
}

#[test]
// Implementation aid: socket sessions cannot survive a host restart.
fn restarted_host_rejects_old_socket_session() {
    let repo = repository(); let mut host = start(repo.path());
    let mut socket = Socket::connect(&host); let ready = socket.frame();
    let old_cookie = cookie(&host);
    let port = host.address().rsplit(':').next().unwrap().to_owned();
    drop(socket); host.stop_gracefully(Duration::from_secs(15));
    let mut command = review_host_support::review_command(repo.path()); command.args(["--port", &port]);
    let restarted = review_host_support::start_command(command);
    assert_eq!(upgrade(&restarted, "/review-events", &format!("Cookie: {old_cookie}\r\nOrigin: {}\r\n", restarted.config["endpoint"].as_str().unwrap())).0, 401);
    let mut current = Socket::connect(&restarted);
    assert_ne!(current.frame()["epoch"], ready["epoch"]);
}
