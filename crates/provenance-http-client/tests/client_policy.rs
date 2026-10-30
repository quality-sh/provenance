//! The Rust client runs the response policy cases of the shared Promise client harness:
//! malformed responses, the response size limit, an oversized response stream, and a lost
//! mutation response.
use provenance_http_client::{
    types::AnswerQuestionRequest, Error, HttpClient, COMPATIBILITY, MAX_RESPONSE_BYTES,
};
use serde_json::{json, Value};
use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

type Reply = Box<dyn FnOnce(&mut TcpStream) + Send>;

fn metadata() -> String {
    let (wire, state, review_journal, read_derivation) = COMPATIBILITY;
    json!({
        "data": {
            "compatibility": {
                "wire": wire, "state": state,
                "review_journal": review_journal, "read_derivation": read_derivation
            },
            "package": {"name": "fixture", "version": "0"},
            "repository": "fixture", "scope": "default"
        },
        "meta": {}
    })
    .to_string()
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap_or(0)
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let read = stream.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        let text = String::from_utf8_lossy(&bytes);
        if let Some(end) = text.find("\r\n\r\n") {
            if bytes.len() >= end + 4 + content_length(&text[..end]) {
                break;
            }
        }
    }
    String::from_utf8(bytes).unwrap()
}

fn respond(status: u16, body: String) -> Reply {
    Box::new(move |stream| {
        write!(
            stream,
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    })
}

/// Streams up to four times the response bound. `closed` is set when the client closes
/// the connection before the host ends the stream.
fn unbounded(closed: Arc<AtomicBool>) -> Reply {
    Box::new(move |stream| {
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        let chunk = vec![b' '; 1024 * 1024];
        for _ in 0..4 * MAX_RESPONSE_BYTES / chunk.len() {
            if let Err(error) = stream.write_all(&chunk) {
                assert!(
                    !matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
                    "the client stopped reading but kept the connection open"
                );
                closed.store(true, Ordering::SeqCst);
                return;
            }
        }
    })
}

/// Reads the request and closes the connection without a response.
fn lost() -> Reply {
    Box::new(|_| {})
}

struct Host {
    url: String,
    stop: Arc<AtomicBool>,
    join: thread::JoinHandle<(Vec<String>, usize)>,
}

/// Serves `replies` in order, then accepts and counts each further connection until
/// `finish` stops it.
fn host(replies: Vec<Reply>) -> Host {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    let join = thread::spawn(move || {
        let requests = replies
            .into_iter()
            .map(|reply| {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let request = read_request(&mut stream);
                reply(&mut stream);
                request
            })
            .collect();
        listener.set_nonblocking(true).unwrap();
        let mut extra = 0;
        loop {
            match listener.accept() {
                Ok(_) => extra += 1,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("accept failed: {error}"),
            }
        }
        (requests, extra)
    });
    Host { url, stop, join }
}

impl Host {
    /// Returns each request and asserts that the client opened no further connection.
    /// The join runs off the test runtime so the client can still close its connections.
    async fn finish(self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        let (requests, extra) = tokio::task::spawn_blocking(move || self.join.join().unwrap())
            .await
            .unwrap();
        assert_eq!(extra, 0, "the client sent a request again");
        requests
    }
}

/// Runs one operation. A call that does not end in time fails the test.
async fn call(client: &HttpClient, mutates: bool) -> Result<(), Error> {
    tokio::time::timeout(Duration::from_secs(60), operation(client, mutates))
        .await
        .expect("the call did not end in time")
}

async fn operation(client: &HttpClient, mutates: bool) -> Result<(), Error> {
    if mutates {
        let request: AnswerQuestionRequest =
            serde_json::from_value(json!({"data": {"answer": "Done."}})).unwrap();
        client
            .answer_question("question_x", &request)
            .await
            .map(drop)
    } else {
        client
            .list_requirement_history("requirement_a", None, None)
            .await
            .map(drop)
    }
}

#[tokio::test]
async fn malformed_responses_are_refused_for_reads_and_mutations() {
    for mutates in [false, true] {
        for status in [200, 400] {
            let body = json!({"private_body": "secret-host-details"}).to_string();
            let host = host(vec![respond(200, metadata()), respond(status, body)]);
            let client = HttpClient::connect(&host.url).await.unwrap();
            let error = call(&client, mutates).await.unwrap_err();
            assert!(
                matches!(error, Error::MalformedResponse(_)),
                "status {status}, mutation {mutates}: {error}"
            );
            assert!(!format!("{error} {error:?}").contains("secret-host-details"));
            assert_eq!(host.finish().await.len(), 2);
        }
    }
}

/// Sends `body` with a declared length and ignores a write error, because the client can
/// close the connection before it reads the last byte.
fn sized(status: u16, body: Vec<u8>) -> Reply {
    Box::new(move |stream| {
        let head = format!(
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.write_all(&body));
    })
}

/// A declared failure padded with spaces to `size` bytes.
fn failure_of_size(size: usize) -> Vec<u8> {
    let mut body = json!({"error": {"kind": "unauthenticated"}, "meta": {}})
        .to_string()
        .into_bytes();
    body.resize(size, b' ');
    body
}

#[tokio::test]
async fn a_response_at_the_limit_is_read_and_one_byte_more_is_refused() {
    let policy: Value =
        serde_json::from_str(include_str!("../src/client-policy-cases.json")).unwrap();
    let limit = usize::try_from(policy["max_response_bytes"].as_u64().unwrap()).unwrap();
    for mutates in [false, true] {
        let at_limit = host(vec![
            respond(200, metadata()),
            sized(401, failure_of_size(limit)),
        ]);
        let client = HttpClient::connect(&at_limit.url).await.unwrap();
        let error = call(&client, mutates).await.unwrap_err();
        assert!(
            matches!(error, Error::Operation { status: 401, .. }),
            "mutation {mutates}: {error}"
        );
        assert_eq!(at_limit.finish().await.len(), 2);

        let over_limit = host(vec![
            respond(200, metadata()),
            sized(401, failure_of_size(limit + 1)),
        ]);
        let client = HttpClient::connect(&over_limit.url).await.unwrap();
        let error = call(&client, mutates).await.unwrap_err();
        assert!(
            matches!(error, Error::MalformedResponse(_)),
            "mutation {mutates}: {error}"
        );
        assert_eq!(over_limit.finish().await.len(), 2);
    }
}

#[tokio::test]
async fn an_oversized_response_is_refused_and_its_stream_closed() {
    let policy: Value =
        serde_json::from_str(include_str!("../src/client-policy-cases.json")).unwrap();
    assert_eq!(
        usize::try_from(policy["max_response_bytes"].as_u64().unwrap()).unwrap(),
        MAX_RESPONSE_BYTES
    );
    for mutates in [false, true] {
        let closed = Arc::new(AtomicBool::new(false));
        let host = host(vec![
            respond(200, metadata()),
            unbounded(Arc::clone(&closed)),
        ]);
        let client = HttpClient::connect(&host.url).await.unwrap();
        let error = call(&client, mutates).await.unwrap_err();
        assert!(
            matches!(error, Error::MalformedResponse(_)),
            "mutation {mutates}: {error}"
        );
        drop(client);
        assert_eq!(host.finish().await.len(), 2);
        assert!(closed.load(Ordering::SeqCst), "mutation {mutates}");
    }
}

#[tokio::test]
async fn a_lost_mutation_response_is_a_connection_error_and_is_not_sent_again() {
    let host = host(vec![respond(200, metadata()), lost()]);
    let client = HttpClient::connect(&host.url).await.unwrap();
    let error = call(&client, true).await.unwrap_err();
    assert!(matches!(error, Error::Connection(_)), "{error}");
    let requests = host.finish().await;
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("POST /questions/question_x/answer "));
}
