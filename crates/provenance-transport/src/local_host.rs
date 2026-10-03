//! Publishes and verifies credential-free local host locations.

use anyhow::Context as _;
use fs2::FileExt as _;
use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

pub const IDENTITY_ROUTE: &str = "/local-host-identity";
const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalHostIdentity {
    pub schema_version: u32,
    pub repository_id: String,
    pub scope: String,
    pub instance_nonce: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RegistryRecord {
    schema_version: u32,
    endpoint: String,
    repository_id: String,
    scope: String,
    instance_nonce: String,
}

impl RegistryRecord {
    fn identity(&self) -> LocalHostIdentity {
        LocalHostIdentity {
            schema_version: self.schema_version,
            repository_id: self.repository_id.clone(),
            scope: self.scope.clone(),
            instance_nonce: self.instance_nonce.clone(),
        }
    }
}

#[derive(Debug)]
pub struct LocalHostRegistration {
    path: PathBuf,
    lock_path: PathBuf,
    identity: LocalHostIdentity,
}

impl LocalHostRegistration {
    pub fn publish(
        root: &Path,
        scope: &str,
        endpoint: &str,
        repository_id: &str,
    ) -> anyhow::Result<Self> {
        let root = canonical_root(root)?;
        validate_scope(scope)?;
        validate_endpoint(endpoint).context("local host endpoint is not a loopback base URL")?;
        let path = registry_path(&root, scope);
        let directory = path.parent().expect("registry file has a parent");
        secure_directory(directory)?;
        let lock_path = lock_path(&root, scope);
        let lock = lock(&lock_path)?;
        if let Some(existing) = read_secure(&path) {
            anyhow::ensure!(
                !verified(&existing),
                "repository and scope already has a running local host"
            );
        }
        let record = RegistryRecord {
            schema_version: SCHEMA_VERSION,
            endpoint: endpoint.to_owned(),
            repository_id: repository_id.to_owned(),
            scope: scope.to_owned(),
            instance_nonce: uuid::Uuid::new_v4().simple().to_string(),
        };
        replace(&path, &record)?;
        fs2::FileExt::unlock(&lock)?;
        Ok(Self {
            path,
            lock_path,
            identity: record.identity(),
        })
    }

    pub fn identity(&self) -> LocalHostIdentity {
        self.identity.clone()
    }
}

impl Drop for LocalHostRegistration {
    fn drop(&mut self) {
        let Ok(lock) = lock(&self.lock_path) else {
            return;
        };
        if read_record(&self.path).is_some_and(|record| {
            record.instance_nonce == self.identity.instance_nonce
        }) {
            let _ = std::fs::remove_file(&self.path);
        }
        let _ = fs2::FileExt::unlock(&lock);
    }
}

#[derive(Clone, Debug)]
pub struct DiscoveredLocalHost {
    endpoint: url::Url,
}

impl DiscoveredLocalHost {
    pub const fn endpoint(&self) -> &url::Url {
        &self.endpoint
    }
}

/// Returns the live local host whose identity matches the secure registry record.
#[rule("rule_review_link_opens_repository_host_only")]
pub fn discover(root: &Path, scope: &str) -> anyhow::Result<Option<DiscoveredLocalHost>> {
    let root = canonical_root(root)?;
    validate_scope(scope)?;
    let path = registry_path(&root, scope);
    let Some(record) = read_secure(&path) else {
        return Ok(None);
    };
    if record.schema_version == SCHEMA_VERSION && record.scope == scope && verified(&record) {
        return Ok(Some(DiscoveredLocalHost {
            endpoint: validate_endpoint(&record.endpoint)?,
        }));
    }
    remove_stale(&root, scope, &record.instance_nonce)?;
    Ok(None)
}

fn canonical_root(root: &Path) -> anyhow::Result<PathBuf> {
    root.canonicalize()
        .with_context(|| format!("cannot canonicalize repository root {}", root.display()))
}

fn validate_scope(scope: &str) -> anyhow::Result<()> {
    provenance_core::ScopeId::new(scope).map(|_| ())
}

fn validate_endpoint(endpoint: &str) -> anyhow::Result<url::Url> {
    let url = url::Url::parse(endpoint)?;
    anyhow::ensure!(url.scheme() == "http", "scheme must be http");
    anyhow::ensure!(url.username().is_empty(), "userinfo is not allowed");
    anyhow::ensure!(url.password().is_none(), "userinfo is not allowed");
    anyhow::ensure!(matches!(url.path(), "" | "/"), "path is not allowed");
    anyhow::ensure!(url.query().is_none(), "query is not allowed");
    anyhow::ensure!(url.fragment().is_none(), "fragment is not allowed");
    anyhow::ensure!(url.port().is_some_and(|port| port != 0), "a nonzero port is required");
    let loopback = match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    };
    anyhow::ensure!(loopback, "host must be a loopback IP address");
    Ok(url)
}

fn verified(record: &RegistryRecord) -> bool {
    let Ok(mut url) = validate_endpoint(&record.endpoint) else {
        return false;
    };
    url.set_path(IDENTITY_ROUTE);
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_millis(200))
        .timeout_read(Duration::from_millis(200))
        .timeout_write(Duration::from_millis(200))
        .build();
    let Ok(response) = agent.get(url.as_str()).call() else {
        return false;
    };
    let Ok(text) = response.into_string() else {
        return false;
    };
    serde_json::from_str::<LocalHostIdentity>(&text)
        .is_ok_and(|identity| identity == record.identity())
}

fn registry_path(root: &Path, scope: &str) -> PathBuf {
    root.join(".provenance/cache/local-hosts")
        .join(format!("{scope}.json"))
}

fn lock_path(root: &Path, scope: &str) -> PathBuf {
    root.join(".provenance/cache/local-hosts")
        .join(format!("{scope}.lock"))
}

fn secure_directory(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("cannot create local host directory {}", path.display()))?;
    #[cfg(unix)]
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn lock(path: &Path) -> anyhow::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options.open(path)?;
    #[cfg(unix)]
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.lock_exclusive()?;
    Ok(file)
}

fn read_secure(path: &Path) -> Option<RegistryRecord> {
    secure_to_read(path.parent().expect("registry file has a parent"), path)?;
    read_record(path)
}

fn read_record(path: &Path) -> Option<RegistryRecord> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn replace(path: &Path, record: &RegistryRecord) -> anyhow::Result<()> {
    let temporary = path.with_file_name(format!(".local-host-{}.tmp", record.instance_nonce));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary)?;
    let result = (|| {
        file.write_all(&serde_json::to_vec(record)?)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        #[cfg(unix)]
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok::<_, anyhow::Error>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

fn remove_stale(root: &Path, scope: &str, nonce: &str) -> anyhow::Result<()> {
    let path = registry_path(root, scope);
    let lock = lock(&lock_path(root, scope))?;
    if read_record(&path).is_some_and(|record| record.instance_nonce == nonce) {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    fs2::FileExt::unlock(&lock)?;
    Ok(())
}

fn secure_to_read(directory: &Path, path: &Path) -> Option<()> {
    let directory_metadata = std::fs::symlink_metadata(directory).ok()?;
    let file_metadata = std::fs::symlink_metadata(path).ok()?;
    if !directory_metadata.is_dir()
        || directory_metadata.file_type().is_symlink()
        || !file_metadata.is_file()
        || file_metadata.file_type().is_symlink()
    {
        return None;
    }
    #[cfg(unix)]
    if directory_metadata.permissions().mode() & 0o077 != 0
        || file_metadata.permissions().mode() & 0o077 != 0
    {
        return None;
    }
    Some(())
}
