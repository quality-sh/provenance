use provenance_http_client::{types::CheckStatementRequest, Error, HttpClient, OperationFailure};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

fn metadata(wire: u32) -> String {
    serde_json::json!({
        "data": {
            "compatibility": {"wire":wire,"state":2,"review_journal":3,"read_derivation":3},
            "package": {"name":"fixture","version":"0"},
            "repository": "fixture", "scope": "default"
        },
        "meta": {}
    })
    .to_string()
}

fn host(bodies: Vec<(u16, String)>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let join = thread::spawn(move || {
        bodies.into_iter().map(|(status, body)| {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut buffer = [0; 8192];
        let read = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]).to_string();
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        request
    }).collect()
    });
    (url, join)
}

#[tokio::test]
async fn complete_tuple_mismatch_stops_at_metadata() {
    let (url, join) = host(vec![(200, metadata(0))]);
    assert!(matches!(
        HttpClient::connect(&url).await,
        Err(Error::CompatibilityMismatch)
    ));
    let requests = join.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /metadata "));
}

#[tokio::test]
async fn bound_identity_mismatch_stops_at_metadata() {
    let (url, join) = host(vec![(200, metadata(9))]);
    assert!(matches!(
        HttpClient::connect_with_bound_identity(&url, "token", "other", "default").await,
        Err(Error::IdentityMismatch)
    ));
    let requests = join.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /metadata "));
}

#[tokio::test]
async fn resource_call_uses_v2_path_and_preserves_typed_failure() {
    let failure = serde_json::json!({
        "error":{"kind":"invalid_input","field":"statement","reason":"required"}, "meta":{}
    });
    let (url, join) = host(vec![(200, metadata(9)), (400, failure.to_string())]);
    let client = HttpClient::connect(&url).await.unwrap();
    let call: CheckStatementRequest =
        serde_json::from_value(serde_json::json!({"data":{"statement":""}})).unwrap();
    match client.check_statement(&call).await.unwrap_err() {
        Error::Operation {
            status: 400,
            failure: OperationFailure::CheckStatement(actual),
        } => {
            assert_eq!(serde_json::to_value(actual).unwrap(), failure);
        }
        error => panic!("expected typed statement failure, got {error}"),
    }
    let requests = join.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("POST /statement-checks "));
    assert!(requests[1].contains("{\"data\":{\"statement\":\"\"}}"));
}
