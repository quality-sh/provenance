//! Paths outside the asset bundle answer like the host API, with a JSON failure envelope.
#[path = "review_host_support/mod.rs"]
mod review_host_support;

use review_host_support::{repository, request, response, start};
use serde_json::Value;

#[test]
/// Implementation aid: an SDK that hits a route this host does not know needs the same
/// authenticated refusal envelope as the host API, not a bare status.
fn unknown_paths_get_the_authenticated_route_refusals() {
    let repo = repository();
    let host = start(repo.path());
    for (method, path, auth, status, kind) in [
        ("GET", "/nonexistent", false, 401, "unauthenticated"),
        ("POST", "/nonexistent", false, 401, "unauthenticated"),
        ("POST", "/", false, 401, "unauthenticated"),
        ("GET", "/nonexistent", true, 404, "unknown_operation"),
        ("POST", "/nonexistent", true, 404, "unknown_operation"),
        ("DELETE", "/assets/missing.js", true, 404, "unknown_operation"),
        ("POST", "/index.html", true, 405, "method_not_allowed"),
        ("POST", "/", true, 405, "method_not_allowed"),
        ("PUT", "/requirements", true, 405, "method_not_allowed"),
    ] {
        let reply = response(request(&host, method, path, auth).call());
        assert_eq!(reply.status(), status, "{method} {path} auth={auth}");
        assert_eq!(
            reply.header("Content-Type"),
            Some("application/json"),
            "{method} {path}"
        );
        let body: Value = serde_json::from_str(&reply.into_string().unwrap()).unwrap();
        assert_eq!(body["error"]["kind"], kind, "{method} {path} auth={auth}");
    }
    for method in ["GET", "HEAD"] {
        let page = request(&host, method, "/", false).call().unwrap();
        assert_eq!(page.status(), 200, "{method} / stays an open asset");
    }
}
