#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{repository, request, response, start};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
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
#[verifies("rule_review_refusal_reaches_client", examples)]
/// A refusal sent before the body is read does not reset the connection,
/// so a client that still sends its body gets the response.
fn a_refusal_before_the_body_does_not_reset_the_connection() {
    let repo = repository();
    let host = start(repo.path());
    let address = host.config["endpoint"]
        .as_str()
        .unwrap()
        .trim_start_matches("http://")
        .to_owned();
    let body = vec![b'x'; 1024 * 1024];
    let mut stream = TcpStream::connect(&address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "POST /requirements/req_example/discussions HTTP/1.1\r\nHost: {address}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();

    // The host refuses and ends its side before it reads the body.
    let mut refusal = Vec::new();
    stream.read_to_end(&mut refusal).unwrap();
    assert!(
        refusal.starts_with(b"HTTP/1.1 401 "),
        "{}",
        String::from_utf8_lossy(&refusal)
    );

    // The body that follows is read and dropped, not answered with a reset.
    stream.write_all(&body).expect("body after the refusal");
    stream.shutdown(Shutdown::Write).unwrap();
    let mut rest = Vec::new();
    stream
        .read_to_end(&mut rest)
        .expect("the host closes without a reset");
    assert!(rest.is_empty());
}
