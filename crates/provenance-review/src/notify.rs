//! The client side of review notifications: it sends a notify request to the running review
//! host, which checks the target and queues the messages.

use crate::{launch_key, RepositoryScope};
use provenance_porcelain::get::{
    Notify, NotifyPort, PortFuture, ReadError, Registration, NO_REVIEW_HOST,
};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};

/// The host route that registers a client thread for review changes.
pub const DELIVERIES_ROUTE: &str = "/review-deliveries";

// Registration reads the thread and queues the full document before the host replies.
const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(90);

/// Sends notify requests to the review host of one repository and scope.
pub struct HostNotifyPort {
    place: RepositoryScope,
}

impl HostNotifyPort {
    pub const fn new(place: RepositoryScope) -> Self {
        Self { place }
    }
}

impl NotifyPort for HostNotifyPort {
    fn register<'a>(
        &'a self,
        root: &'a str,
        limit: usize,
        notify: &'a Notify,
    ) -> PortFuture<'a, Registration> {
        let place = self.place.clone();
        let body = json!({
            "rootId": root,
            "limit": limit,
            "notify": notify,
            "codexHome": codex_home().map(|path| path.to_string_lossy().into_owned()),
        });
        Box::pin(async move {
            tokio::task::spawn_blocking(move || register(&place, body))
                .await
                .map_err(|error| ReadError::Operation(error.to_string()))?
        })
    }
}

fn register(place: &RepositoryScope, mut body: Value) -> Result<Registration, ReadError> {
    let operation = |error: anyhow::Error| ReadError::Operation(format!("{error:#}"));
    let host = place
        .host()
        .map_err(operation)?
        .ok_or_else(|| ReadError::Operation(NO_REVIEW_HOST.to_owned()))?;
    let key = launch_key::read(host.instance_nonce()).map_err(operation)?;
    body["repositoryId"] = json!(host.repository_id());
    body["scope"] = json!(place.scope);
    body["instanceNonce"] = json!(host.instance_nonce());
    let mut url = host.endpoint().clone();
    url.set_path(DELIVERIES_ROUTE);
    let reply = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(REGISTRATION_TIMEOUT)
        .build()
        .post(url.as_str())
        .set("Content-Type", "application/json")
        .set("x-provenance-launch-key", &key)
        .send_string(&body.to_string());
    let text = match reply {
        Ok(response) => response.into_string(),
        Err(ureq::Error::Status(_, response)) => {
            let text = response.into_string().unwrap_or_default();
            let refusal: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let error = &refusal["error"];
            let message = error["message"]
                .as_str()
                .unwrap_or("the review host refused the notify request")
                .to_owned();
            return Err(match error["field"].as_str() {
                Some(field) if error["kind"] == "invalid_input" => ReadError::InvalidInput {
                    field: field.to_owned(),
                    message,
                },
                _ if error["kind"] == "host_busy" => ReadError::HostBusy(message),
                _ => ReadError::Operation(message),
            });
        }
        Err(error) => return Err(operation(anyhow::Error::new(error))),
    }
    .map_err(|error| operation(error.into()))?;
    serde_json::from_str(&text).map_err(|error| operation(error.into()))
}

/// The Codex home that this process uses: `CODEX_HOME`, else `.codex` in the home directory.
pub fn codex_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME").filter(|home| !home.is_empty()) {
        return Some(PathBuf::from(home));
    }
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(|home| PathBuf::from(home).join(".codex"))
}
