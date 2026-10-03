//! Publishes review host locations without their credentials.

use anyhow::Context;
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningHost {
    pub endpoint: String,
    pub repository_id: String,
    pub scope: String,
    pub instance_nonce: String,
}

#[derive(Default, Deserialize, Serialize)]
struct Registry {
    hosts: Vec<RunningHost>,
}

pub struct HostRuntime {
    path: PathBuf,
    lock_path: PathBuf,
    instance_nonce: String,
}

impl HostRuntime {
    pub fn publish(
        repo: &Path,
        scope: &str,
        endpoint: &str,
        repository_id: &str,
    ) -> anyhow::Result<Self> {
        validate_endpoint(endpoint).context("review host endpoint is not a loopback base URL")?;
        let path = path(repo, scope);
        let directory = path.parent().expect("runtime file has a parent");
        secure_directory(directory)?;
        let lock_path = lock_path(repo, scope);
        let instance_nonce = uuid::Uuid::new_v4().simple().to_string();
        let lock = lock(&lock_path)?;
        let mut registry = read_registry(&path).unwrap_or_default();
        registry.hosts.push(RunningHost {
            endpoint: endpoint.to_owned(),
            repository_id: repository_id.to_owned(),
            scope: scope.to_owned(),
            instance_nonce: instance_nonce.clone(),
        });
        replace(&path, &registry, &instance_nonce)?;
        fs2::FileExt::unlock(&lock)?;
        Ok(Self {
            path,
            lock_path,
            instance_nonce,
        })
    }

    pub fn instance_nonce(&self) -> &str {
        &self.instance_nonce
    }
}

impl Drop for HostRuntime {
    fn drop(&mut self) {
        let Ok(lock) = lock(&self.lock_path) else {
            return;
        };
        let Some(mut registry) = read_registry(&self.path) else {
            return;
        };
        registry
            .hosts
            .retain(|host| host.instance_nonce != self.instance_nonce);
        if registry.hosts.is_empty() {
            let _ = std::fs::remove_file(&self.path);
        } else {
            let _ = replace(&self.path, &registry, &self.instance_nonce);
        }
        let _ = fs2::FileExt::unlock(&lock);
    }
}

/// Reads a complete secure registry. Invalid state is stale state.
pub fn read(repo: &Path, scope: &str) -> Option<Vec<RunningHost>> {
    let path = path(repo, scope);
    secure_to_read(path.parent().expect("runtime file has a parent"), &path)?;
    read_registry(&path).map(|registry| registry.hosts)
}

pub fn validate_endpoint(endpoint: &str) -> anyhow::Result<url::Url> {
    let url = url::Url::parse(endpoint)?;
    anyhow::ensure!(url.scheme() == "http", "scheme must be http");
    anyhow::ensure!(url.username().is_empty(), "userinfo is not allowed");
    anyhow::ensure!(url.password().is_none(), "userinfo is not allowed");
    anyhow::ensure!(matches!(url.path(), "" | "/"), "path is not allowed");
    anyhow::ensure!(url.query().is_none(), "query is not allowed");
    anyhow::ensure!(url.fragment().is_none(), "fragment is not allowed");
    anyhow::ensure!(url.port().is_some(), "an explicit port is required");
    let loopback = match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    };
    anyhow::ensure!(loopback, "host must be a loopback IP address");
    Ok(url)
}

fn path(repo: &Path, scope: &str) -> PathBuf {
    repo.join(".provenance/cache/review-hosts")
        .join(format!("{scope}.json"))
}

fn lock_path(repo: &Path, scope: &str) -> PathBuf {
    repo.join(".provenance/cache/review-hosts")
        .join(format!("{scope}.lock"))
}

fn secure_directory(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("cannot create review host runtime directory {}", path.display()))?;
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

fn read_registry(path: &Path) -> Option<Registry> {
    let bytes = std::fs::read(path).ok()?;
    let registry: Registry = serde_json::from_slice(&bytes).ok()?;
    (!registry.hosts.is_empty()).then_some(registry)
}

fn replace(path: &Path, registry: &Registry, nonce: &str) -> anyhow::Result<()> {
    let temporary = path.with_file_name(format!(".review-host-{nonce}.tmp"));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary)?;
    let result = (|| {
        file.write_all(&serde_json::to_vec(registry)?)?;
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
