//! The remote adapter against stand-in objects: a `TcpListener` that reads
//! one request and answers it with bytes that the test gives.

use super::*;
use crate::storage::{Atomicity, StateKey};
use provenance_macros::verifies;
use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

/// The URL of a stand-in object for one connection. It reads the whole
/// request, then gives the connection to `serve`.
fn stand_in(serve: impl FnOnce(TcpStream) + Send + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        serve(stream);
    });
    url
}

fn read_request(stream: &mut TcpStream) {
    let mut head = Vec::new();
    let mut byte = [0];
    while !head.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    let length = String::from_utf8(head)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .map_or(0, |length| length.parse().unwrap());
    stream.read_exact(&mut vec![0; length]).unwrap();
}

/// A stand-in object that writes `raw` and then closes the connection.
fn answering(raw: impl Into<Vec<u8>>) -> String {
    let raw = raw.into();
    stand_in(move |mut stream| stream.write_all(&raw).unwrap())
}

/// A stand-in object that writes `raw` and then keeps the connection open
/// for 15 s.
fn answering_and_holding(raw: impl Into<Vec<u8>>) -> String {
    let raw = raw.into();
    stand_in(move |mut stream| {
        stream.write_all(&raw).unwrap();
        std::thread::sleep(Duration::from_secs(15));
    })
}

fn storage(url: &str) -> RemoteStateStorage {
    RemoteStateStorage::new(url, "p", "main").unwrap()
}

fn publish_to(url: &str) -> Result<(), PublishFailure> {
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(StateKey::new("manifest.json").unwrap(), b"{}".to_vec());
    storage(url).publish(writes)
}

fn empty_state_body() -> String {
    serde_json::to_string(&StateBody {
        revision: Revision::of(&BTreeMap::new()).as_str().to_owned(),
        entries: BTreeMap::new(),
    })
    .unwrap()
}

/// The raw answer `200` with the empty state, in chunks or with its length.
fn empty_state(chunked: bool) -> Vec<u8> {
    let body = empty_state_body();
    if chunked {
        let (first, second) = body.split_at(5);
        format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{first}\r\n{:x};x=1\r\n{second}\r\n0\r\nTrailer: t\r\n\r\n",
            first.len(),
            second.len()
        )
    } else {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
    }
    .into_bytes()
}

#[test]
#[verifies("rule_failure_after_landing_starts_is_an_unknown_outcome", examples)]
fn a_publish_whose_answer_is_lost_is_an_unknown_outcome() {
    for raw in [
        &b""[..],
        b"HTTP/1.1 204",
        b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n{}",
    ] {
        let outcome = publish_to(&answering(raw));
        assert!(
            matches!(outcome, Err(PublishFailure::Started(_))),
            "{:?}: {outcome:?}",
            String::from_utf8_lossy(raw)
        );
    }
}

#[test]
#[verifies("rule_failure_after_landing_starts_is_an_unknown_outcome", examples)]
fn a_publish_answered_by_an_unexplained_5xx_is_an_unknown_outcome() {
    let html = "<html><body>Bad Gateway</body></html>";
    let raw = format!(
        "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{html}",
        html.len()
    );
    let outcome = publish_to(&answering(raw));
    assert!(
        matches!(outcome, Err(PublishFailure::Started(_))),
        "{outcome:?}"
    );
}

/// An implementation aid with no Rule binding: no Rule says which answers
/// refuse a set. An answer of `4xx` says that the object did not take the
/// request, so no key landed.
#[test]
fn a_publish_answered_by_an_unexplained_4xx_is_refused() {
    let text = "Payload Too Large";
    let raw = format!(
        "HTTP/1.1 413 Payload Too Large\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{text}",
        text.len()
    );
    let outcome = publish_to(&answering(raw));
    assert!(
        matches!(outcome, Err(PublishFailure::Refused(_))),
        "{outcome:?}"
    );
}

/// An implementation aid with no Rule binding: it keeps the adapter from
/// waiting for a close that a proxy can hold back.
#[test]
fn an_answer_ends_at_its_length_while_the_connection_stays_open() {
    let started = Instant::now();
    publish_to(&answering_and_holding(
        &b"HTTP/1.1 204 No Content\r\n\r\n"[..],
    ))
    .unwrap();
    for chunked in [false, true] {
        let view = storage(&answering_and_holding(empty_state(chunked)))
            .read_view()
            .unwrap();
        assert!(view.entries().is_empty());
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}

/// An implementation aid with no Rule binding: an object can send interim
/// answers before the final one.
#[test]
fn interim_1xx_answers_are_skipped() {
    let raw = "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 103 Early Hints\r\nLink: </a>\r\n\r\nHTTP/1.1 204 No Content\r\n\r\n";
    publish_to(&answering(raw)).unwrap();
}

/// An implementation aid with no Rule binding: two lengths that differ make
/// the end of the body unknown, so the answer is not read.
#[test]
fn an_answer_with_two_different_content_lengths_is_refused() {
    let raw = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 3\r\n\r\n{}x";
    let Err(StorageError::Read(error)) = storage(&answering(raw)).read_view() else {
        panic!("an answer with two lengths gave a view");
    };
    assert!(format!("{error:#}").contains("Content-Length"), "{error:#}");
    let outcome = publish_to(&answering(raw));
    assert!(
        matches!(outcome, Err(PublishFailure::Started(_))),
        "{outcome:?}"
    );
    let body = empty_state_body();
    let length = body.len();
    let same = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\ncontent-length: {length}\r\n\r\n{body}"
    );
    storage(&answering(same)).read_view().unwrap();
}

/// An implementation aid with no Rule binding: an object base with a space,
/// a line break, `?` or `#` would break the request line or the `Host`
/// header.
#[test]
fn a_base_with_a_space_a_line_break_a_query_or_a_fragment_is_refused() {
    for base in [
        "http://a/b c",
        "http://a/x\r\nX-Evil: 1",
        "http://a\n/x",
        "http://a\t",
        "http://a/x?y",
        "http://a/x#y",
    ] {
        assert!(Address::parse(base).is_err(), "{base:?}");
        assert!(
            RemoteStateStorage::new(base, "p", "main").is_err(),
            "{base:?}"
        );
    }
}

/// An implementation aid with no Rule binding: it checks how a base becomes
/// the address to connect to, the `Host` header and the path prefix.
#[test]
fn a_base_gives_its_host_port_and_path_prefix() {
    let address = Address::parse("http://objects.internal/kernel/").unwrap();
    assert_eq!(address.connect, "objects.internal:80");
    assert_eq!(address.host, "objects.internal");
    assert_eq!(address.prefix, "/kernel");
    let address = Address::parse("http://127.0.0.1:8080").unwrap();
    assert_eq!(address.connect, "127.0.0.1:8080");
    assert_eq!(address.prefix, "");
    for base in ["https://a", "http://", "http://u@a", "a:1"] {
        assert!(Address::parse(base).is_err(), "{base}");
    }
}

/// An implementation aid with no Rule binding: a name goes into the path
/// as it is, so a name that needs encoding is refused.
#[test]
fn the_storage_refuses_a_bad_project_or_session_name() {
    assert!(RemoteStateStorage::new("http://a", "a/b", "main").is_err());
    assert!(RemoteStateStorage::new("http://a", "a", "").is_err());
    let storage = RemoteStateStorage::new("http://a/base", "p", "main").unwrap();
    assert_eq!(storage.session, "/base/projects/p/sessions/main");
}
