#[path = "review_host_support/mod.rs"]
mod review_host_support;
use provenance_macros::verifies;
use review_host_support::{
    events::{upgrade, upgrade_head, Socket},
    repository, session, start,
};
use std::time::Duration;

#[test]
/// A browser socket opens with one exact Origin and the page session as a subprotocol; the
/// host names provenance-review in its answer.
#[verifies("rule_review_events_need_page_session", examples)]
fn a_page_session_subprotocol_opens_the_event_socket() {
    let repo = repository();
    let host = start(repo.path());
    let session = session(&host);
    let origin = host.config["endpoint"].as_str().unwrap();
    let (response, mut socket) = upgrade_head(
        &host,
        "/review-events",
        &format!("Sec-WebSocket-Protocol: provenance-review, {session}\r\nOrigin: {origin}\r\n"),
        host.address(),
    );
    assert!(response.starts_with("HTTP/1.1 101 "), "{response}");
    assert!(response
        .to_ascii_lowercase()
        .contains("\r\nsec-websocket-protocol: provenance-review\r\n"));
    assert_eq!(socket.frame()["type"], "ready");
}

#[test]
/// The host bearer, an Authorization header, a cookie, or a missing, doubled or changed
/// subprotocol entry gets a refusal before the upgrade.
#[verifies("rule_review_events_need_page_session", examples)]
fn event_upgrade_requires_the_session_origin_and_destination() {
    let repo = repository();
    let host = start(repo.path());
    let mut valid = Socket::connect(&host);
    assert_eq!(valid.frame()["type"], "ready");
    let session = session(&host);
    let bearer = host.bearer();
    let nonce = host.config["instanceNonce"].as_str().unwrap();
    let origin = host.config["endpoint"].as_str().unwrap();
    let offer = format!("Sec-WebSocket-Protocol: provenance-review, {session}\r\n");
    for headers in [
        format!("Origin: {origin}\r\n"),
        format!("Authorization: Bearer {bearer}\r\nOrigin: {origin}\r\n"),
        format!("Authorization: Bearer {session}\r\nOrigin: {origin}\r\n"),
        format!("{offer}Authorization: Bearer {bearer}\r\nOrigin: {origin}\r\n"),
        format!("Cookie: provenance_review_session_{nonce}={session}\r\nOrigin: {origin}\r\n"),
        format!("Sec-WebSocket-Protocol: provenance-review, {bearer}\r\nOrigin: {origin}\r\n"),
        format!("Sec-WebSocket-Protocol: {session}\r\nOrigin: {origin}\r\n"),
        format!("Sec-WebSocket-Protocol: provenance-review\r\nOrigin: {origin}\r\n"),
        format!("Sec-WebSocket-Protocol: provenance-review, {session}changed\r\nOrigin: {origin}\r\n"),
        format!("Sec-WebSocket-Protocol: provenance-review, {session}, {session}\r\nOrigin: {origin}\r\n"),
        format!("{offer}{offer}Origin: {origin}\r\n"),
        format!("{offer}"),
        format!("{offer}Origin: {origin}\r\nOrigin: {origin}\r\n"),
        format!("{offer}Origin: http://foreign.test\r\n"),
        format!("{offer}Origin: {origin}\r\nHost: {}\r\n", host.address()),
        format!("{offer}Origin: {origin}\r\nSec-Fetch-Site: cross-site\r\n"),
        format!("{offer}Origin: {origin}\r\nSec-Fetch-Site: same-origin\r\nSec-Fetch-Site: same-origin\r\n"),
    ] {
        let (status, _) = upgrade(&host, "/review-events", &headers);
        assert!(matches!(status, 400 | 401 | 403), "refusal before upgrade, got {status} for {headers}");
    }
    let headers = format!("{offer}Origin: {origin}\r\n");
    for path in [
        "/review-events?token=secret",
        "/review-events?bearer=secret",
        "/review-events?launch=secret",
    ] {
        assert_eq!(upgrade(&host, path, &headers).0, 403);
    }
}

#[test]
/// A session from an earlier host run cannot open a socket after a restart on the same port.
#[verifies("rule_review_events_need_page_session", examples)]
fn restarted_host_rejects_old_socket_session() {
    let repo = repository();
    let mut host = start(repo.path());
    let mut socket = Socket::connect(&host);
    let ready = socket.frame();
    let old_session = session(&host);
    let port = host.address().rsplit(':').next().unwrap().to_owned();
    drop(socket);
    host.stop_gracefully(Duration::from_secs(15));
    let mut command = review_host_support::review_command(repo.path());
    command.args(["--port", &port]);
    let restarted = review_host_support::start_command(command);
    assert_eq!(
        upgrade(
            &restarted,
            "/review-events",
            &format!(
                "Sec-WebSocket-Protocol: provenance-review, {old_session}\r\nOrigin: {}\r\n",
                restarted.config["endpoint"].as_str().unwrap()
            )
        )
        .0,
        401
    );
    let mut current = Socket::connect(&restarted);
    assert_ne!(current.frame()["epoch"], ready["epoch"]);
}
