use provenance_http_client::HttpClient;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

fn host(body: Value, status: u16) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        for payload in [
            json!({"engine_version":"fixture","protocol_version":7}),
            body,
        ] {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let text = String::from_utf8(request).unwrap();
            let length = text
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|value| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            socket.read_exact(&mut vec![0; length]).unwrap();
            let code = if text.starts_with("GET ") {
                200
            } else {
                status
            };
            let body = payload.to_string();
            write!(socket, "HTTP/1.1 {code} response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    (url, worker)
}

#[tokio::test]
async fn omitted_required_nullable_field_is_a_malformed_response() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../contracts/operations/fixtures.openapi.json"
    ))
    .unwrap();
    let mut body = fixtures["x-wire-fixtures"]["EvidenceOutput"].clone();
    body.as_object_mut().unwrap().remove("stale");
    let (url, worker) = host(body, 200);
    let client = HttpClient::connect(&url).await.unwrap();
    let call = serde_json::from_value(json!({"context":{"repository":"fixture","scope":"default"},"request":{"rule":"rule_shared"}})).unwrap();
    let result = client.evidence(&call).await;
    assert!(
        result.is_err(),
        "serde Option must not weaken a required nullable field"
    );
    assert!(format!("{:?}", result.unwrap_err()).starts_with("MalformedResponse"));
    worker.join().unwrap();
}

#[tokio::test]
async fn malformed_success_and_refusal_after_write_are_uncertain() {
    for status in [200, 400] {
        let (url, worker) = host(json!({"secret":"must not escape"}), status);
        let client = HttpClient::connect(&url).await.unwrap();
        let call = serde_json::from_value(json!({"context":{"repository":"fixture","scope":"default"},"request":{"schema_version":2,"spec":"fixture","declared_by":"fixture"}})).unwrap();
        let error = client.apply(&call).await.unwrap_err();
        assert!(matches!(
            error,
            provenance_http_client::Error::UncertainWrite { .. }
        ));
        assert!(!format!("{error:?} {error}").contains("must not escape"));
        worker.join().unwrap();
    }
}

#[tokio::test]
async fn validated_write_refusal_is_typed_but_internal_outcomes_are_uncertain() {
    let policy: Value =
        serde_json::from_str(include_str!("../src/client-policy-cases.json")).unwrap();
    for case in policy["refusals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["mutates"] == true)
    {
        let kind = case["kind"].as_str().unwrap();
        let uncertain = case["uncertain"].as_bool().unwrap();
        let (url, worker) = host(
            json!({"protocol_version":7,"operation":"complete-verification","error":{"kind":kind}}),
            400,
        );
        let client = HttpClient::connect(&url).await.unwrap();
        let call = serde_json::from_value(json!({"context":{"repository":"fixture","scope":"default"},"request":{"run":"run_fixture","status":"passed"}})).unwrap();
        let error = client.complete_verification(&call).await.unwrap_err();
        if uncertain {
            assert!(
                matches!(error, provenance_http_client::Error::UncertainWrite { .. }),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(error, provenance_http_client::Error::Operation { .. }),
                "{error:?}"
            );
        }
        worker.join().unwrap();
    }
}
