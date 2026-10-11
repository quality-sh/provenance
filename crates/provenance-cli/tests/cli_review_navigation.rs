#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{repository, request, response};
use serde_json::Value;

#[test]
#[verifies("rule_review_link_opens_signed_in", examples)]
fn private_redirect_navigation_loads_the_review_document() {
    let repo = repository();
    let host = review_host_support::start(repo.path());
    for path in ["/", "/index.html?root=req_example"] {
        for method in ["GET", "HEAD"] {
            let reply = request(&host, method, path, false)
                .set("Sec-Fetch-Site", "cross-site")
                .set("Sec-Fetch-Mode", "navigate")
                .set("Sec-Fetch-Dest", "document")
                .call().unwrap();
            assert_eq!(reply.status(), 200);
            assert_eq!(reply.header("Content-Type"), Some("text/html; charset=utf-8"));
            assert_eq!(reply.header("Cache-Control"), Some("no-store"));
            assert_eq!(reply.header("X-Frame-Options"), Some("DENY"));
            assert!(reply.header("Content-Security-Policy").unwrap().contains("frame-ancestors 'none'"));
            if method == "GET" {
                assert!(reply.into_string().unwrap().contains("<!doctype html>"));
            }
        }
    }
}

#[test]
/// Implementation aid: the navigation exception must not permit data access or a foreign Host.
fn cross_site_navigation_keeps_access_checks() {
    let repo = repository();
    let host = review_host_support::start(repo.path());
    for (method, path, wrong_host, mode, dest, origin) in [
        ("GET", "/", true, "navigate", "document", None),
        ("GET", "/", false, "navigate", "document", Some("null")),
        ("GET", "/", false, "cors", "document", None),
        ("GET", "/", false, "navigate", "script", None),
        ("POST", "/", false, "navigate", "document", None),
        ("GET", "/review-config", false, "navigate", "document", None),
        ("GET", "/requirements/req_absent/document", false, "navigate", "document", None),
        ("POST", "/review-launch/code", false, "navigate", "document", None),
        ("POST", "/review-launch/session", false, "navigate", "document", None),
        ("GET", "/review-launch/code", false, "navigate", "document", None),
        ("GET", "/review-launch/session", false, "navigate", "document", None),
        ("GET", "/review.js", false, "navigate", "document", None),
        ("GET", "/review.css", false, "navigate", "document", None),
        ("GET", "/metadata", false, "navigate", "document", None),
        ("GET", "/local-host-identity", false, "navigate", "document", None),
    ] {
        let mut call = request(&host, method, path, true)
            .set("Sec-Fetch-Site", "cross-site")
            .set("Sec-Fetch-Mode", mode)
            .set("Sec-Fetch-Dest", dest);
        if wrong_host { call = call.set("Host", "unrelated.test"); }
        if let Some(origin) = origin { call = call.set("Origin", origin); }
        let reply = response(call.call());
        assert_eq!(reply.status(), 403, "{method} {path}");
        assert_eq!(reply.header("Cache-Control"), Some("no-store"));
        let body: Value = serde_json::from_str(&reply.into_string().unwrap()).unwrap();
        assert_eq!(body["error"]["kind"], "access_denied");
    }
}
