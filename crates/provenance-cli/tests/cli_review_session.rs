#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{redeem, repository, request, response, session, start, Host};
use serde_json::{json, Value};
use std::time::Duration;

fn page_get(host: &Host, path: &str) -> ureq::Request {
    request(host, "GET", path, false)
        .set("Sec-Fetch-Site", "same-origin")
        .set("Origin", host.config["endpoint"].as_str().unwrap())
}

#[test]
/// The real host exchanges a launch code for a page session in the body, with no cookie.
#[verifies("rule_review_host_accepts_no_cookie", examples)]
fn the_exchange_returns_the_session_and_sets_no_cookie() {
    let repo = repository();
    let host = start(repo.path());
    let key = std::fs::read_to_string(host.launch_key_path()).unwrap();
    let issued = request(&host, "POST", "/review-launch/code", false)
        .send_string(&json!({"launchKey": key}).to_string())
        .unwrap();
    let issued: Value = serde_json::from_str(&issued.into_string().unwrap()).unwrap();
    let exchanged = redeem(&host, issued["code"].as_str().unwrap());
    assert_eq!(exchanged.status(), 200);
    assert_eq!(exchanged.header("Set-Cookie"), None);
    let body: Value = serde_json::from_str(&exchanged.into_string().unwrap()).unwrap();
    let session = body["session"].as_str().unwrap();
    let nonce = host.config["instanceNonce"].as_str().unwrap();
    let secret = session.strip_prefix(&format!("{nonce}.")).unwrap();
    assert_eq!(secret.len(), 64);
    assert!(secret.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(!session.contains(host.bearer()));
    assert_eq!(body.as_object().unwrap().len(), 1);
}

#[test]
/// The real host retains access after a reload and in a new tab through the stored session.
#[verifies("rule_review_link_opens_signed_in", examples)]
fn the_session_keeps_access_after_reload_and_in_a_new_tab() {
    let repo = repository();
    let host = start(repo.path());
    let session = session(&host);
    for _ in 0..2 {
        assert_eq!(
            request(&host, "GET", "/", false).call().unwrap().status(),
            200
        );
        for path in ["/review-config", "/metadata", "/discussion-containers"] {
            assert_eq!(
                page_get(&host, path)
                    .set("Authorization", &format!("Bearer {session}"))
                    .call()
                    .unwrap()
                    .status(),
                200
            );
        }
    }
}

#[test]
/// A cookie alone grants nothing, even one that holds the session or the host bearer.
#[verifies("rule_review_host_accepts_no_cookie", examples)]
fn a_cookie_only_request_is_refused() {
    let repo = repository();
    let host = start(repo.path());
    let session = session(&host);
    let nonce = host.config["instanceNonce"].as_str().unwrap();
    for cookie in [
        format!("provenance_review_session_{nonce}={session}"),
        format!("session={session}"),
        format!("bearer={}", host.bearer()),
    ] {
        assert_eq!(
            response(
                page_get(&host, "/review-config")
                    .set("Cookie", &cookie)
                    .call()
            )
            .status(),
            401
        );
    }
    // A cookie beside the session changes nothing.
    assert_eq!(
        page_get(&host, "/review-config")
            .set("Cookie", &format!("session={session}"))
            .set("Authorization", &format!("Bearer {session}"))
            .call()
            .unwrap()
            .status(),
        200
    );
}

#[test]
/// Implementation aid: the session must not bypass destination or browser origin checks.
fn the_session_refuses_foreign_requests() {
    let repo = repository();
    let host = start(repo.path());
    let session = session(&host);
    for (header, value) in [
        ("Host", "foreign.test"),
        ("Origin", "http://foreign.test"),
        ("Sec-Fetch-Site", "cross-site"),
        ("Sec-Fetch-Site", "same-site"),
    ] {
        assert_eq!(
            response(
                request(&host, "GET", "/review-config", false)
                    .set("Authorization", &format!("Bearer {session}"))
                    .set(header, value)
                    .call()
            )
            .status(),
            403
        );
    }
}

#[test]
/// Implementation aid: a new host run must not accept a previous session.
fn a_restarted_host_refuses_the_session() {
    let repo = repository();
    let mut host = start(repo.path());
    let session = session(&host);
    let port = host.address().rsplit(':').next().unwrap().to_owned();
    host.stop_gracefully(Duration::from_secs(10));
    let mut command = review_host_support::review_command(repo.path());
    command.args(["--port", &port]);
    let restarted = review_host_support::start_command(command);
    assert_ne!(
        host.config["instanceNonce"],
        restarted.config["instanceNonce"]
    );
    assert_eq!(
        response(
            request(&restarted, "GET", "/review-config", false)
                .set("Authorization", &format!("Bearer {session}"))
                .call()
        )
        .status(),
        401
    );
}

#[test]
/// Implementation aid: ambiguous or changed sessions must not grant access.
fn invalid_sessions_refuse_access() {
    let repo = repository();
    let host = start(repo.path());
    let session = session(&host);
    for value in [
        format!("Bearer {session}changed"),
        format!("bearer {session}"),
        session.clone(),
    ] {
        assert_eq!(
            response(
                request(&host, "GET", "/review-config", false)
                    .set("Authorization", &value)
                    .call()
            )
            .status(),
            401
        );
    }
}

#[test]
/// Implementation aid: a failed exchange returns no session and sets no cookie.
fn a_refused_exchange_returns_no_session() {
    let repo = repository();
    let host = start(repo.path());
    let refused = redeem(&host, "not-a-launch-code");
    assert_eq!(refused.status(), 401);
    assert_eq!(refused.header("Set-Cookie"), None);
    assert!(!refused.into_string().unwrap().contains("session\""));
}

#[test]
/// Implementation aid: concurrent hosts must refuse each other's sessions.
fn concurrent_hosts_keep_their_own_sessions() {
    let first_repo = repository();
    let second_repo = repository();
    let first = start(first_repo.path());
    let second = start(second_repo.path());
    let first_session = session(&first);
    let second_session = session(&second);
    assert_ne!(first_session, second_session);
    for (host, own, foreign) in [
        (&first, &first_session, &second_session),
        (&second, &second_session, &first_session),
    ] {
        assert_eq!(
            page_get(host, "/review-config")
                .set("Authorization", &format!("Bearer {own}"))
                .call()
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            response(
                page_get(host, "/review-config")
                    .set("Authorization", &format!("Bearer {foreign}"))
                    .call()
            )
            .status(),
            401
        );
    }
}
