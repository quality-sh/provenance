#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{events::upgrade, follow::create, repository, scripted_feed, start};
use serde_json::json;

#[test]
// Implementation aid: a sequence gap from an independent peer must re-read saved state.
fn sequence_gap_returns_reset_even_when_the_document_has_not_changed() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let feed = scripted_feed::Feed::start(repo.path());
    feed.send(json!({"type":"ready", "epoch":feed.epoch, "sequence":0}));
    let follow = feed.follow(repo.path());
    assert_eq!(follow.frame()["type"], "state");
    feed.send(json!({"type":"changed", "epoch":feed.epoch, "sequence":2, "scopes":[]}));
    let reset = follow.frame();
    assert_eq!(reset["type"], "reset");
    assert_eq!(reset["sequence"], 2);
    assert_eq!(reset["continuity"], "unknown");
    assert!(reset["records"].to_string().contains("req_follow"));
}

#[test]
// Implementation aid: an independent restarted peer must reset a saved client baseline.
fn changed_host_epoch_returns_reset_with_current_state() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let feed = scripted_feed::Feed::start(repo.path());
    feed.send(json!({"type":"ready", "epoch":feed.epoch, "sequence":0}));
    let follow = feed.follow(repo.path());
    follow.frame();
    // The sequence is contiguous, so only the epoch change can cause this reset.
    feed.send(json!({"type":"ready", "epoch":"restarted-host", "sequence":1}));
    let reset = follow.frame();
    assert_eq!(reset["type"], "reset");
    assert_eq!(reset["epoch"], "restarted-host");
    assert_eq!(reset["sequence"], 1);
    assert_eq!(reset["continuity"], "unknown");
    assert!(reset["records"].to_string().contains("req_follow"));
}

#[test]
// Implementation aid: reject unknown host tags instead of treating them as invalidations.
fn unknown_host_frame_is_a_protocol_error() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let feed = scripted_feed::Feed::start(repo.path());
    feed.send(json!({"type":"ready", "epoch":feed.epoch, "sequence":0}));
    let follow = feed.follow(repo.path());
    follow.frame();
    feed.send(json!({"type":"unrecognized", "epoch":feed.epoch, "sequence":1}));
    let error = follow.frame();
    assert_eq!(error["type"], "disconnected");
    assert_eq!(error["continuity"], "unknown");
}

#[test]
// Implementation aid: a broken output pipe must close the WebSocket before command failure.
fn output_failure_closes_the_host_socket() {
    use std::process::Stdio;
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let feed = scripted_feed::Feed::start(repo.path());
    feed.send(json!({"type":"ready", "epoch":feed.epoch, "sequence":0}));
    let mut child = feed
        .command(repo.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let closed = feed
        .closed
        .recv_timeout(review_host_support::waits::LIMIT)
        .unwrap();
    assert!(
        closed,
        "output failure must send a Close frame, not only TCP EOF"
    );
    assert!(!child.wait().unwrap().success());
}

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
    for (headers, expected) in [
        ("X-Provenance-Launch-Key: wrong\r\n".to_owned(), 401),
        (format!("{header}{header}"), 401),
        (format!("{header}Cookie: arbitrary=value\r\n"), 403),
        (format!("{header}Sec-Fetch-Site: same-origin\r\n"), 403),
        (format!("{header}Sec-Fetch-Mode: websocket\r\n"), 403),
        (format!("{header}Sec-Fetch-Dest: empty\r\n"), 403),
        (format!("{header}Authorization: Bearer unused\r\n"), 403),
        (
            format!(
                "{header}Origin: {}\r\n",
                host.config["endpoint"].as_str().unwrap()
            ),
            403,
        ),
    ] {
        let (status, _) = upgrade(&host, "/review-events", &headers);
        assert_eq!(status, expected, "headers: {headers}");
    }
    let (status, _) = upgrade(&host, "/review-events?key=unused", &header);
    assert_eq!(status, 403);
}

#[test]
// Implementation aid: a native key cannot authorize a DNS rebinding Host header.
fn native_subscription_refuses_a_wrong_host() {
    let repo = repository();
    let host = start(repo.path());
    let key = std::fs::read_to_string(host.launch_key_path()).unwrap();
    let (status, _) = review_host_support::events::upgrade_with_host(
        &host,
        "/review-events",
        &format!("X-Provenance-Launch-Key: {key}\r\n"),
        "example.com",
    );
    assert_eq!(status, 403);
}
