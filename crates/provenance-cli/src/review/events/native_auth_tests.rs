//! Implementation aid: exercise native refusals through the public event route in process.
use super::ReviewEvents;
use axum::http::HeaderValue;
use provenance_store::{layout::ProvenanceLayout, repository_init::plan_manifest};
use provenance_transport::LocalAccess;
use std::sync::Arc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Error};

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
    let mut request = format!("ws://{address}/review-events")
        .into_client_request()
        .unwrap();
    if valid_key {
        request
            .headers_mut()
            .insert("x-provenance-launch-key", value.parse().unwrap());
    }
    for (name, value) in extra {
        request.headers_mut().append(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_bytes(value).unwrap(),
        );
    }
    if let Some(host) = host {
        request.headers_mut().insert("host", host.parse().unwrap());
    }
    let result = tokio_tungstenite::connect_async(request).await;
    let status = match result {
        Ok((socket, response)) => {
            drop(socket);
            response.status().as_u16()
        }
        Err(Error::Http(response)) => response.status().as_u16(),
        Err(error) => panic!("unexpected handshake error: {error}"),
    };
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
