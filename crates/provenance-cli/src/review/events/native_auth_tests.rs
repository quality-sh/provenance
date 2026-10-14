//! Implementation aid: exercise native refusals through the public event route in process.
use super::ReviewEvents;
use provenance_store::{layout::ProvenanceLayout, repository_init::plan_manifest};
use provenance_transport::LocalAccess;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

async fn status(extra: &[(&str, &[u8])], valid_key: bool, host: Option<&str>) -> u16 {
    let directory = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(directory.path().to_str().unwrap());
    let manifest = plan_manifest(None, Some("default"), None, vec![], false, None).unwrap();
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let access = Arc::new(
        LocalAccess::new(
            directory.path(),
            "native",
            "default",
            &"0".repeat(64),
            address,
        )
        .unwrap(),
    );
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let key = crate::review::launch::LaunchKey::publish(&nonce).unwrap();
    let value = crate::review::launch::LaunchKey::read(&nonce).unwrap();
    let codes = crate::review::launch::LaunchCodes::new(&"0".repeat(64), key);
    let events = ReviewEvents::start(layout, "default".into(), nonce, access, codes).await;
    let router = events.router();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut request = format!(
        "GET /review-events HTTP/1.1\r\nHost: {}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n",
        host.map_or_else(|| address.to_string(), str::to_owned)
    ).into_bytes();
    if valid_key {
        request.extend_from_slice(format!("X-Provenance-Launch-Key: {value}\r\n").as_bytes());
    }
    for (name, value) in extra {
        request.extend_from_slice(name.as_bytes());
        request.extend_from_slice(b": ");
        request.extend_from_slice(value);
        request.extend_from_slice(b"\r\n");
    }
    request.extend_from_slice(b"\r\n");
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket.write_all(&request).await.unwrap();
    let mut response = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        BufReader::new(socket).read_line(&mut response),
    )
    .await
    .unwrap()
    .unwrap();
    let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
    events.shutdown().await;
    server.abort();
    status
}

macro_rules! refuses_browser_header {
    ($name:ident, $header:literal) => {
        #[tokio::test]
        async fn $name() {
            assert_eq!(status(&[($header, b"unused")], true, None).await, 403);
        }
    };
}

refuses_browser_header!(refuses_origin, "origin");
refuses_browser_header!(refuses_cookie, "cookie");
refuses_browser_header!(refuses_authorization, "authorization");
refuses_browser_header!(refuses_fetch_site, "sec-fetch-site");
refuses_browser_header!(refuses_fetch_mode, "sec-fetch-mode");
refuses_browser_header!(refuses_fetch_destination, "sec-fetch-dest");

#[tokio::test]
async fn refuses_wrong_key() {
    assert_eq!(
        status(&[("x-provenance-launch-key", b"wrong")], false, None).await,
        401
    );
}

#[tokio::test]
async fn refuses_duplicate_key() {
    assert_eq!(
        status(&[("x-provenance-launch-key", b"wrong")], true, None).await,
        401
    );
}

#[tokio::test]
async fn refuses_non_text_key() {
    assert_eq!(
        status(&[("x-provenance-launch-key", b"\xff")], false, None).await,
        401
    );
}

#[tokio::test]
async fn refuses_wrong_host() {
    assert_eq!(status(&[], true, Some("example.com")).await, 403);
}

#[tokio::test]
async fn accepts_native_key() {
    assert_eq!(status(&[], true, None).await, 101);
}
