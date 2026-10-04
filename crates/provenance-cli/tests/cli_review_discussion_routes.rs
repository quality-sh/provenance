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
        assert_ne!(
            response(request(&host, "GET", &path, true).call()).status(),
            404
        );
    }
    for family in ["domains", "boundaries"] {
        let path = format!("/{family}/absent/discussions");
        assert_eq!(
            response(request(&host, "GET", &path, true).call()).status(),
            404
        );
        assert_eq!(
            response(request(&host, "POST", &path, true).send_string("{}")).status(),
            404
        );
    }
}
