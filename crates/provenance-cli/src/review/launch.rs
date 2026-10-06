//! Single-use launch codes that sign a review page in without a pasted credential.

use anyhow::Context as _;
use axum::{
    body::Bytes,
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use provenance_core::protocol::failure::OperationFailure;
use provenance_transport::constant_time_eq;
use serde::Deserialize;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

const CODE_LIFETIME: Duration = Duration::from_secs(120);
const MAX_LIVE_CODES: usize = 64;

/// The secret that a local CLI sends to get a launch code. Only the per-user cache holds it.
pub struct LaunchKey {
    path: PathBuf,
    value: String,
}

impl LaunchKey {
    /// Writes a new key into an owner-only file that names this host instance.
    pub fn publish(nonce: &str) -> anyhow::Result<Self> {
        let path = key_path(nonce)?;
        let directory = path.parent().expect("key file has a parent");
        std::fs::create_dir_all(directory)
            .with_context(|| format!("cannot create {}", directory.display()))?;
        #[cfg(unix)]
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
        let value = secret();
        let file = crate::owner_file::create(&path);
        let mut file = file
            .with_context(|| format!("cannot create the review launch key {}", path.display()))?;
        std::io::Write::write_all(&mut file, value.as_bytes())?;
        file.sync_all()?;
        Ok(Self { path, value })
    }

    /// Reads the key of a running host instance from the per-user cache.
    pub fn read(nonce: &str) -> anyhow::Result<String> {
        let path = key_path(nonce)?;
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
            let mut file =
                std::fs::File::open(&path).context("cannot read the review launch key")?;
            crate::owner_file::verify(&file)?;
            let mut value = String::new();
            std::io::Read::read_to_string(&mut file, &mut value)
                .context("cannot read the review launch key")?;
            value
        };
        anyhow::ensure!(is_secret(&value), "the review launch key is not valid");
        Ok(value)
    }
}

impl Drop for LaunchKey {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn key_path(nonce: &str) -> anyhow::Result<PathBuf> {
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

fn secret() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn is_secret(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Issues launch codes to the key holder and exchanges each code for the bearer one time.
#[derive(Clone)]
pub struct LaunchCodes {
    inner: Arc<Inner>,
}

struct Inner {
    bearer: String,
    key: LaunchKey,
    live: Mutex<Vec<IssuedCode>>,
}

struct IssuedCode {
    value: String,
    expires_at: Instant,
}

impl LaunchCodes {
    pub fn new(bearer: &str, key: LaunchKey) -> Self {
        Self {
            inner: Arc::new(Inner {
                bearer: bearer.to_owned(),
                key,
                live: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Issues a code that is valid for one exchange during the code lifetime.
    pub fn issue(&self, now: Instant) -> String {
        let mut live = self.inner.live.lock().expect("launch code lock");
        live.retain(|code| code.expires_at > now);
        if live.len() >= MAX_LIVE_CODES {
            live.remove(0);
        }
        let value = secret();
        live.push(IssuedCode {
            value: value.clone(),
            expires_at: now + CODE_LIFETIME,
        });
        value
    }

    /// Returns the bearer for a live code and removes the code.
    pub fn redeem(&self, code: &str, now: Instant) -> Option<String> {
        let mut live = self.inner.live.lock().expect("launch code lock");
        live.retain(|issued| issued.expires_at > now);
        let mut found = None;
        for (index, issued) in live.iter().enumerate() {
            if constant_time_eq(issued.value.as_bytes(), code.as_bytes()) {
                found = Some(index);
            }
        }
        live.remove(found?);
        drop(live);
        Some(self.inner.bearer.clone())
    }

    fn holds_key(&self, supplied: &str) -> bool {
        constant_time_eq(self.inner.key.value.as_bytes(), supplied.as_bytes())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeRequest {
    launch_key: String,
}

#[derive(Deserialize)]
struct SessionRequest {
    code: String,
}

pub(super) async fn issue_code(State(codes): State<LaunchCodes>, body: Bytes) -> Response {
    match serde_json::from_slice::<CodeRequest>(&body) {
        Ok(request) if codes.holds_key(&request.launch_key) => {
            Json(json!({ "code": codes.issue(Instant::now()) })).into_response()
        }
        _ => super::refusal(OperationFailure::Unauthenticated),
    }
}

pub(super) async fn open_session(State(codes): State<LaunchCodes>, body: Bytes) -> Response {
    let bearer = serde_json::from_slice::<SessionRequest>(&body)
        .ok()
        .and_then(|request| codes.redeem(&request.code, Instant::now()));
    bearer.map_or_else(
        || super::refusal(OperationFailure::Unauthenticated),
        |bearer| Json(json!({ "bearer": bearer })).into_response(),
    )
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
