use provenance_http_client::{Error, HttpClient, PROTOCOL_VERSION};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

fn host(bodies: Vec<(u16, String)>) -> (String, thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let join = thread::spawn(move || {
        let mut count = 0;
        for (status, body) in bodies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut buffer = [0; 4096];
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0);
            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            count += 1;
        }
        count
    });
    (url, join)
}

#[tokio::test]
async fn metadata_mismatch_prevents_operation_submission() {
    let (url, join) = host(vec![(
        200,
        "{\"engine_version\":\"test\",\"protocol_version\":0}".into(),
    )]);
    assert!(matches!(
        HttpClient::connect(&url).await,
        Err(Error::ProtocolMismatch {
            expected: PROTOCOL_VERSION,
            received: 0
        })
    ));
    assert_eq!(join.join().unwrap(), 1);
}

#[tokio::test]
async fn typed_refusal_survives_without_retry() {
    let failure = serde_json::json!({"protocol_version":PROTOCOL_VERSION,"operation":"check-statement",
        "error":{"kind":"invalid_input","field":"statement","reason":"required"}});
    let (url, join) = host(vec![
        (
            200,
            format!("{{\"engine_version\":\"test\",\"protocol_version\":{PROTOCOL_VERSION}}}"),
        ),
        (400, failure.to_string()),
    ]);
    let client = HttpClient::connect(&url).await.unwrap();
    let call =
        serde_json::from_value(serde_json::json!({"request":{"statement":"Stop."}})).unwrap();
    match client.check_statement(&call).await.unwrap_err() {
        Error::Operation {
            status,
            failure: actual,
        } => {
            assert_eq!(status, 400);
            assert_eq!(serde_json::to_value(actual).unwrap(), failure);
        }
        error => panic!("expected typed refusal, received {error}"),
    }
    assert_eq!(join.join().unwrap(), 2);
}
