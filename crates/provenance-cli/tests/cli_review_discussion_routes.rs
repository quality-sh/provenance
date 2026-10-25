#[path = "review_host_support/mod.rs"]
#[allow(dead_code)]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{repository, request, response, start};

#[test]
#[verifies("rule_domain_boundary_accept_no_discussions", exhaustion)]
fn served_discussion_routes_exclude_domain_and_boundary_parents() {
    let repo = repository();
    let host = start(repo.path());
    for family in [
        "sources",
        "requirements",
        "resolutions",
        "rules",
        "topics",
        "questions",
    ] {
        let path = format!("/{family}/absent/discussions");
        assert_eq!(
            response(request(&host, "POST", &path, true).send_string("{}")).status(),
            400
        );
    }
    // No discussion route exists for these families, so the host API answers unknown_operation.
    for family in ["domains", "boundaries"] {
        let path = format!("/{family}/absent/discussions");
        let reply = response(request(&host, "POST", &path, true).send_string("{}"));
        assert_eq!(reply.status(), 404, "{path}");
        let body: serde_json::Value = serde_json::from_str(&reply.into_string().unwrap()).unwrap();
        assert_eq!(body["error"]["kind"], "unknown_operation", "{path}");
    }
}
