use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, HeaderValue, Request},
};
use provenance_core::{Manifest, RepoPathPrefix, RequirementStatus, ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateRequirementInput, StateStore},
};
use provenance_transport::{HostAccess, LocalAccess, StatementHost};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Arc};
use tower::ServiceExt;

const TOKEN: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    StateStore::new(layout)
        .create_requirement(CreateRequirementInput {
            scope_id: scope,
            id: StableId::new("req_local").unwrap(),
            statement: "The graph is readable.".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: None,
            depends_on: vec![],
            supersedes: vec![],
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    dir
}

fn access(root: &Path) -> LocalAccess {
    LocalAccess::new(
        root,
        "A",
        "default",
        TOKEN,
        "127.0.0.1:43210".parse().unwrap(),
    )
    .unwrap()
}

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("127.0.0.1:43210"));
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap(),
    );
    headers
}

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name().into_string().unwrap();
        if path.is_dir() {
            files.insert(format!("{name}/"), vec![]);
            files.extend(
                snapshot(&path)
                    .into_iter()
                    .map(|(child, bytes)| (format!("{name}/{child}"), bytes)),
            );
        } else {
            files.insert(name, std::fs::read(path).unwrap());
        }
    }
    files
}

async fn get(host: &StatementHost, target: &str, scope: &str) -> (u16, Value) {
    let mut request = Request::post("/v7/operations/get")
        .body(Body::from(
            json!({
                "context":{"repository":target,"scope":scope,"freshness":"catch_up"},
                "request":{"node_type":"requirement","id":"req_local"}
            })
            .to_string(),
        ))
        .unwrap();
    *request.headers_mut() = headers();
    let response = host.router().oneshot(request).await.unwrap();
    (
        response.status().as_u16(),
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap(),
    )
}

#[tokio::test]
async fn grants_precede_storage_and_authorized_reads_prepare_the_projection() {
    let repo = repository();
    let other = repository();
    let host = StatementHost::with_access(Arc::new(access(repo.path())));
    let before = snapshot(repo.path());
    let other_before = snapshot(other.path());
    for (target, scope, status) in [
        ("B", "default", 404),
        (other.path().to_str().unwrap(), "default", 404),
        ("A", "other", 403),
    ] {
        assert_eq!(get(&host, target, scope).await.0, status);
        assert_eq!(snapshot(repo.path()), before);
        assert_eq!(snapshot(other.path()), other_before);
    }
    let (status, result) = get(&host, "A", "default").await;
    assert_eq!(status, 200, "{result}");
    assert!(
        result.to_string().contains("The graph is readable."),
        "{result}"
    );
    host.shutdown().await;
}

#[test]
fn rejects_duplicate_headers_cross_site_requests_and_invalid_configuration() {
    let repo = repository();
    let access = access(repo.path());
    assert!(access.authenticate(&headers()).is_ok());
    for header in ["host", "authorization", "origin", "sec-fetch-site"] {
        let mut headers = headers();
        if header == "origin" {
            headers.insert(header, HeaderValue::from_static("http://127.0.0.1:43210"));
        }
        if header == "sec-fetch-site" {
            headers.insert(header, HeaderValue::from_static("same-origin"));
        }
        let value = headers[header].clone();
        headers.append(header, value);
        assert!(access.authenticate(&headers).is_err(), "{header}");
    }
    for site in ["cross-site", "same-site"] {
        let mut headers = headers();
        headers.insert("sec-fetch-site", HeaderValue::from_str(site).unwrap());
        assert!(access.authenticate(&headers).is_err());
    }
    for (id, scope, token, address) in [
        ("", "default", TOKEN, "127.0.0.1:1234"),
        ("../repo", "default", TOKEN, "127.0.0.1:1234"),
        ("A", "missing", TOKEN, "127.0.0.1:1234"),
        ("A", "default", "short", "127.0.0.1:1234"),
        ("A", "default", TOKEN, "0.0.0.0:1234"),
        ("A", "default", TOKEN, "127.0.0.1:0"),
    ] {
        assert!(LocalAccess::new(repo.path(), id, scope, token, address.parse().unwrap()).is_err());
    }
}

async fn invoke(
    host: &StatementHost,
    operation: &str,
    body: &Value,
    headers: HeaderMap,
) -> (u16, Value) {
    let mut request = Request::post(format!("/v7/operations/{operation}"))
        .body(Body::from(body.to_string()))
        .unwrap();
    *request.headers_mut() = headers;
    let response = host.router().oneshot(request).await.unwrap();
    (
        response.status().as_u16(),
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap(),
    )
}

#[tokio::test]
async fn denied_valid_reads_and_writes_leave_both_repositories_unchanged() {
    let repo = repository();
    let other = repository();
    let host = StatementHost::with_access(Arc::new(access(repo.path())));
    let before = snapshot(repo.path());
    let other_before = snapshot(other.path());
    for (operation, input) in [
        ("get", json!({"node_type":"requirement","id":"req_local"})),
        (
            "post-thread-message",
            json!({"scope_id":"default", "parent":{
            "node_type":"requirement","node_id":"req_local"},"role":"user","body":"Unauthorized change"}),
        ),
    ] {
        let body = json!({"context":{"repository":"A","scope":"default"},"request":input});
        for (name, value, status) in [
            ("authorization", "", 401),
            ("authorization", "Bearer wrong", 401),
            ("host", "rebind.attacker.test:43210", 403),
            ("origin", "http://127.0.0.1:43211", 403),
            ("origin", "null", 403),
            ("sec-fetch-site", "same-site", 403),
            ("sec-fetch-site", "cross-site", 403),
        ] {
            let mut headers = headers();
            headers.insert(name, HeaderValue::from_str(value).unwrap());
            let (actual, reply) = invoke(&host, operation, &body, headers).await;
            assert_eq!(actual, status, "{operation} {name}: {reply}");
            assert!(!reply.to_string().contains("The graph is readable."));
            assert_eq!(snapshot(repo.path()), before);
            assert_eq!(snapshot(other.path()), other_before);
        }
        for (target, scope, status) in [
            ("B", "default", 404),
            (other.path().to_str().unwrap(), "default", 404),
            ("../B", "default", 404),
            ("A", "../default", 403),
            ("A", "other", 403),
        ] {
            let mut body = body.clone();
            body["context"]["repository"] = json!(target);
            body["context"]["scope"] = json!(scope);
            assert_eq!(invoke(&host, operation, &body, headers()).await.0, status);
            assert_eq!(snapshot(repo.path()), before);
            assert_eq!(snapshot(other.path()), other_before);
        }
    }
    host.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn production_access_keeps_held_file_traversal_checks() {
    let repo = repository();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("outside.rs");
    std::fs::write(&secret, "#[rule(\"rule_secret\")] fn OUTSIDE_SENTINEL() {}").unwrap();
    std::os::unix::fs::symlink(&secret, repo.path().join("link.rs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), repo.path().join("ancestor")).unwrap();
    let host = StatementHost::with_access(Arc::new(access(repo.path())));
    for file in [
        "link.rs",
        "ancestor/outside.rs",
        "../outside.rs",
        "src/../outside.rs",
        "C:\\outside.rs",
        secret.to_str().unwrap(),
    ] {
        let body = json!({"context":{"repository":"A","scope":"default"},"request":{"file":file}});
        let (status, reply) = invoke(&host, "resolve-symbol", &body, headers()).await;
        assert_eq!(status, 403, "{file}: {reply}");
        assert_eq!(reply["error"]["kind"], "file_access_denied");
        assert!(!reply.to_string().contains("OUTSIDE_SENTINEL"));
    }
    host.shutdown().await;
}
