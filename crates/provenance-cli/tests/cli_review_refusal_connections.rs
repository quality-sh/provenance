#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use provenance_transport::MAX_BODY_BYTES;
use review_host_support::{repository, request, response, start};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

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
    let body = vec![b'x'; MAX_BODY_BYTES];
    let mut stream = connect(&host);
    stream.write_all(&request_head(&host, body.len())).unwrap();

    // The host refuses and ends its side before it reads the body.
    assert_full_refusal(&read_refusal(&mut stream));

    // The body that follows is read and dropped, not answered with a reset.
    stream.write_all(&body).expect("body after the refusal");
    stream.shutdown(Shutdown::Write).unwrap();
    let mut rest = Vec::new();
    stream
        .read_to_end(&mut rest)
        .expect("the host closes without a reset");
    assert_eq!(rest, b"");
}

#[test]
#[verifies("rule_review_refusal_reaches_client", examples)]
/// A client that sends part of the body before it reads gets the full refusal.
fn a_refusal_reaches_a_client_that_sent_part_of_the_body() {
    let repo = repository();
    let host = start(repo.path());
    let mut stream = connect(&host);
    let mut request = request_head(&host, MAX_BODY_BYTES);
    request.extend(vec![b'x'; 64 * 1024]);
    stream.write_all(&request).unwrap();

    // The client reads late, so the host has closed before the first read.
    std::thread::sleep(Duration::from_millis(200));
    assert_full_refusal(&read_refusal(&mut stream));
}

#[test]
#[verifies("rule_review_refusal_reaches_client", examples)]
/// A client that sends nothing after the refusal and keeps the socket open
/// does not keep the connection: the host releases it after about 2 seconds.
fn the_host_releases_a_silent_connection_after_2_seconds() {
    let repo = repository();
    let host = start(repo.path());
    let mut stream = connect(&host);
    stream
        .write_all(&request_head(&host, MAX_BODY_BYTES))
        .unwrap();
    assert_full_refusal(&read_refusal(&mut stream));
    let refused = Instant::now();

    // The client sends one byte for each probe. When the host has released the socket, a
    // probe gets a reset, and the next probe fails.
    let released = loop {
        std::thread::sleep(Duration::from_millis(100));
        if stream.write_all(b"x").is_err() {
            break refused.elapsed();
        }
        assert!(
            refused.elapsed() < Duration::from_secs(10),
            "the host keeps a silent connection"
        );
    };
    assert!(
        (Duration::from_millis(1500)..Duration::from_secs(6)).contains(&released),
        "the host released the connection after {released:?}"
    );
}

#[test]
#[verifies("rule_review_refusal_reaches_client", examples)]
/// A client that sends more than the request body limit after the refusal does not keep the
/// connection: the host stops reading at the limit and releases the socket.
fn the_host_stops_reading_at_the_request_body_limit() {
    let repo = repository();
    let host = start(repo.path());
    let mut stream = connect(&host);
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .write_all(&request_head(&host, MAX_BODY_BYTES))
        .unwrap();
    assert_full_refusal(&read_refusal(&mut stream));
    let refused = Instant::now();

    // The socket buffers cannot keep 16 times the limit, so the host must read it or reset.
    let sent = stream.write_all(&vec![b'x'; 16 * MAX_BODY_BYTES]);
    let elapsed = refused.elapsed();
    assert!(sent.is_err(), "the host read more than the limit");
    assert!(
        elapsed < Duration::from_millis(1500),
        "the host stopped reading after {elapsed:?}, not at the limit"
    );
}

fn connect(host: &review_host_support::Host) -> TcpStream {
    let stream = TcpStream::connect(address(host)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
}

fn address(host: &review_host_support::Host) -> String {
    host.config["endpoint"]
        .as_str()
        .unwrap()
        .trim_start_matches("http://")
        .to_owned()
}

/// The head of an unauthenticated POST that announces a body of `length` bytes.
fn request_head(host: &review_host_support::Host, length: usize) -> Vec<u8> {
    format!(
        "POST /requirements/req_example/discussions HTTP/1.1\r\nHost: {}\r\n\
         Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n",
        address(host)
    )
    .into_bytes()
}

/// Reads until the host ends its side of the connection.
fn read_refusal(stream: &mut TcpStream) -> Vec<u8> {
    let mut refusal = Vec::new();
    stream
        .read_to_end(&mut refusal)
        .expect("the refusal arrives without a reset");
    refusal
}

/// Checks that `response` is a complete 401 with the `unauthenticated` refusal kind.
fn assert_full_refusal(response: &[u8]) {
    let text = String::from_utf8_lossy(response);
    let (head, body) = text.split_once("\r\n\r\n").expect("response head");
    assert!(head.starts_with("HTTP/1.1 401 "), "{text}");
    let length: usize = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .expect("Content-Length header");
    assert_eq!(body.len(), length, "{text}");
    let body: serde_json::Value = serde_json::from_str(body).expect("JSON refusal body");
    assert_eq!(body["error"]["kind"], "unauthenticated", "{text}");
}
