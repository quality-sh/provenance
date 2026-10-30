#[path = "review_host_support/mod.rs"]
mod review_host_support;

use review_host_support::{repository, request, response, start};
use std::time::Duration;

#[test]
/// Implementation aid: keeps body-bearing refusals from poisoning reused connections.
fn body_bearing_refusals_do_not_break_the_next_request() {
    let repo = repository();
    let host = start(repo.path());
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    let body = "invalid";

    for _ in 0..4 {
        let refusal = response(
            agent
                .post(&format!(
                    "{}/requirements/req_example/discussions",
                    host.config["endpoint"].as_str().unwrap()
                ))
                .send_string(body),
        );
        assert_eq!(refusal.status(), 401);
        refusal.into_string().expect("refusal response body");
    }

    let valid = response(request(&host, "GET", "/review-config", true).call());
    assert_eq!(valid.status(), 200);
}

#[test]
/// Implementation aid: makes early refusal connection ownership explicit to clients.
fn early_refusals_ask_the_client_to_close_the_connection() {
    let repo = repository();
    let host = start(repo.path());
    let refusal = response(
        ureq::post(&format!(
            "{}/requirements/req_example/discussions",
            host.config["endpoint"].as_str().unwrap()
        ))
        .send_string("invalid"),
    );

    assert_eq!(refusal.status(), 401);
    assert_eq!(refusal.header("Connection"), Some("close"));
}

#[test]
/// Implementation aid: a refused request with a body within the host limit still gets its
/// response, also when the host refuses before it reads the body.
fn a_refused_request_with_a_large_body_gets_its_response() {
    let repo = repository();
    let host = start(repo.path());
    let body = vec![b'x'; 1024 * 1024];

    for (path, auth, expected) in [
        ("/requirements/req_example/discussions", false, 401),
        ("/discussion-containers", false, 401),
        ("/requirements/req_example/discussions", true, 403),
    ] {
        let request = request(&host, "POST", path, auth);
        let request = if auth {
            request.set("Origin", "https://unrelated.test")
        } else {
            request
        };
        let refusal = response(request.send_bytes(&body));
        assert_eq!(refusal.status(), expected, "{path}");
        refusal.into_string().expect("refusal response body");
    }
}
