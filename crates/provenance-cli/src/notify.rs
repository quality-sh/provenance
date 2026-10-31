//! The CLI side of review notifications: it sends a notify request to the running review
//! host, which checks the target and queues the messages.

use crate::repo_context::RepoContext;
use anyhow::Context as _;
use provenance_porcelain::get::{
    Notify, NotifyPort, PortFuture, ReadError, Registration, NO_REVIEW_HOST,
};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

/// The host route that registers a client thread for review changes.
pub const DELIVERIES_ROUTE: &str = "/review-deliveries";

// Registration reads the thread and queues the full document before the host replies.
const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(90);

/// Sends notify requests to the review host of one repository and scope.
pub struct HostNotifyPort {
    context: RepoContext,
}

impl HostNotifyPort {
    pub const fn new(context: RepoContext) -> Self {
        Self { context }
    }
}

impl NotifyPort for HostNotifyPort {
    fn register<'a>(
        &'a self,
        root: &'a str,
        limit: usize,
        notify: &'a Notify,
    ) -> PortFuture<'a, Registration> {
        let context = self.context.clone();
        let body = json!({
            "rootId": root,
            "limit": limit,
            "notify": notify,
            "codexHome": codex_home().map(|path| path.to_string_lossy().into_owned()),
        });
        Box::pin(async move {
            tokio::task::spawn_blocking(move || register(&context, body))
                .await
                .map_err(|error| ReadError::Operation(error.to_string()))?
        })
    }
}

fn register(context: &RepoContext, mut body: Value) -> Result<Registration, ReadError> {
    let operation = |error: anyhow::Error| ReadError::Operation(format!("{error:#}"));
    let host =
        provenance_transport::local_host::discover(context.repo.as_std_path(), &context.scope)
            .map_err(operation)?
            .ok_or_else(|| ReadError::Operation(NO_REVIEW_HOST.to_owned()))?;
    let key = read_launch_key(host.instance_nonce()).map_err(operation)?;
    body["repositoryId"] = json!(host.repository_id());
    body["scope"] = json!(context.scope);
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
            let mut message = error["message"]
                .as_str()
                .unwrap_or("the review host refused the notify request")
                .to_owned();
            if let Some(field) = error["field"].as_str() {
                message = format!("{message} (field {field})");
            }
            return Err(ReadError::Operation(message));
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

/// The owner-only file that holds the launch key of one review host instance.
pub fn launch_key_path(nonce: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        nonce.len() == 32 && nonce.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "the review host instance name is not valid"
    );
    let cache =
        crate::user_cache::cache_directory().context("cannot find the per-user cache directory")?;
    Ok(cache
        .join("provenance")
        .join("review-launch")
        .join(format!("{nonce}.key")))
}

/// Reads the key of a running host instance from the per-user cache.
pub fn read_launch_key(nonce: &str) -> anyhow::Result<String> {
    let path = launch_key_path(nonce)?;
    let metadata = std::fs::symlink_metadata(&path)
        .context("the review host has no launch key for this user")?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "the review launch key is not a regular file"
    );
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        anyhow::bail!("the review launch key is open to other users");
    }
    #[cfg(not(windows))]
    let value = std::fs::read_to_string(&path).context("cannot read the review launch key")?;
    #[cfg(windows)]
    let value = {
        let mut file = std::fs::File::open(&path).context("cannot read the review launch key")?;
        crate::owner_file::verify(&file)?;
        let mut value = String::new();
        std::io::Read::read_to_string(&mut file, &mut value)
            .context("cannot read the review launch key")?;
        value
    };
    anyhow::ensure!(is_secret(&value), "the review launch key is not valid");
    Ok(value)
}

/// Whether a value has the form of a launch key.
pub fn is_secret(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
