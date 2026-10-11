#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{redeem, repository, request, response, start, Host};
use serde_json::{json, Value};
use std::time::Duration;

fn session_cookie(host: &Host) -> String {
    let key = std::fs::read_to_string(host.launch_key_path()).unwrap();
    let issued = request(host, "POST", "/review-launch/code", false)
        .send_string(&json!({"launchKey": key}).to_string())
        .unwrap();
    let issued: Value = serde_json::from_str(&issued.into_string().unwrap()).unwrap();
    let session = redeem(host, issued["code"].as_str().unwrap());
    assert_eq!(session.status(), 200);
    let cookie = session.header("Set-Cookie").expect("session cookie");
    let parts: Vec<_> = cookie.split("; ").collect();
    assert_eq!(&parts[1..], &["HttpOnly", "SameSite=Strict", "Path=/"]);
    assert!(!cookie.contains(host.bearer()));
    assert!(parts[0].starts_with(&format!(
        "provenance_review_session={}.",
        host.config["instanceNonce"].as_str().unwrap()
    )));
    parts[0].to_owned()
}

#[test]
/// Implementation aid: the real host must retain access when page memory is lost.
#[verifies("rule_review_link_opens_signed_in", examples)]
fn cookie_keeps_access_after_reload_and_in_a_new_tab() {
    let repo = repository();
    let host = start(repo.path());
    let cookie = session_cookie(&host);
    for _ in 0..2 {
        assert_eq!(
            request(&host, "GET", "/", false).call().unwrap().status(),
            200
        );
        for path in ["/review-config", "/metadata"] {
            assert_eq!(
                request(&host, "GET", path, false)
                    .set("Cookie", &cookie)
                    .set("Sec-Fetch-Site", "same-origin")
                    .call()
                    .unwrap()
                    .status(),
                200
            );
        }
    }
}

#[test]
/// Implementation aid: a cookie must not bypass destination or browser origin checks.
fn cookie_refuses_foreign_requests() {
    let repo = repository();
    let host = start(repo.path());
    let cookie = session_cookie(&host);
    for (header, value) in [
        ("Host", "foreign.test"),
        ("Origin", "http://foreign.test"),
        ("Sec-Fetch-Site", "cross-site"),
        ("Sec-Fetch-Site", "same-site"),
    ] {
        assert_eq!(
            response(
                request(&host, "GET", "/review-config", false)
                    .set("Cookie", &cookie)
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
fn restarted_host_refuses_the_cookie() {
    let repo = repository();
    let mut host = start(repo.path());
    let cookie = session_cookie(&host);
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
                .set("Cookie", &cookie)
                .call()
        )
        .status(),
        401
    );
}

#[test]
/// Implementation aid: ambiguous or changed cookies must not grant access.
fn invalid_cookies_refuse_access() {
    let repo = repository();
    let host = start(repo.path());
    let cookie = session_cookie(&host);
    for value in [
        format!("{cookie}; {cookie}"),
        format!("{cookie}changed"),
        format!("bearer={}", host.bearer()),
    ] {
        assert_eq!(
            response(
                request(&host, "GET", "/review-config", false)
                    .set("Cookie", &value)
                    .call()
            )
            .status(),
            401
        );
    }
}

#[test]
/// Implementation aid: a failed exchange cannot start or replace a session.
fn a_refused_exchange_sets_no_cookie() {
    let repo = repository();
    let host = start(repo.path());
    let refused = redeem(&host, "not-a-launch-code");
    assert_eq!(refused.status(), 401);
    assert_eq!(refused.header("Set-Cookie"), None);
}
