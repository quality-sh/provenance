//! One call to the HTTP routes of a host, with its whole answer: the
//! status, the headers and the body bytes.

use axum::body::Body;
use axum::http::Request;
use provenance_store::operations::LocalRepository;
use provenance_store::repository::Repository;
use provenance_transport::{HostAccess, OpenCredential, StatementHost};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt as _;

/// The HTTP routes of a host that serves one opened repository under the
/// target name `selected`, as the review host binds it.
pub struct Http(StatementHost);

impl Http {
    /// A host over `repository`.
    pub fn over(repository: Repository) -> Self {
        Self::with_access(Arc::new(
            LocalRepository::bind(repository, "selected").unwrap(),
        ))
    }

    /// A host with `access` as its repository binding.
    pub fn with_access(access: Arc<dyn HostAccess>) -> Self {
        Self(StatementHost::with_access(access).with_credential(Arc::new(OpenCredential)))
    }

    /// Calls `path` with `data` as the request data. `shown` names the call
    /// in the answer.
    pub async fn call(
        &self,
        method: &str,
        path: &str,
        data: Option<Value>,
        headers: &[(&str, &str)],
        shown: String,
    ) -> Answer {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "fixture.test");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        if data.is_some() {
            request = request.header("content-type", "application/json");
        }
        let body = data.map_or_else(Body::empty, |data| {
            Body::from(json!({ "data": data }).to_string())
        });
        let response = self
            .0
            .router()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_owned()))
            .collect();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        Answer {
            shown,
            status,
            headers,
            body,
        }
    }
}

/// The whole answer of one call.
pub struct Answer {
    /// The method and the path, with a generated id in the path shown by
    /// its place.
    pub shown: String,
    pub status: u16,
    /// Each header, in the order of the answer.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Answer {
    /// The body as JSON.
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}
