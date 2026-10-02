//! Publishes the location of a running review host without its credential.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningHost {
    pub endpoint: String,
    pub repository_id: String,
    pub scope: String,
}

pub struct HostRuntime {
    path: PathBuf,
    contents: Vec<u8>,
}

impl HostRuntime {
    pub fn publish(
        repo: &Path,
        scope: &str,
        endpoint: &str,
        repository_id: &str,
    ) -> anyhow::Result<Self> {
        let path = path(repo, scope);
        let directory = path.parent().expect("runtime file has a parent");
        std::fs::create_dir_all(directory).with_context(|| {
            format!("cannot create review host runtime directory {directory:?}")
        })?;
        let contents = serde_json::to_vec(&RunningHost {
            endpoint: endpoint.to_owned(),
            repository_id: repository_id.to_owned(),
            scope: scope.to_owned(),
        })?;
        std::fs::write(&path, &contents)
            .with_context(|| format!("cannot publish review host runtime file {path:?}"))?;
        Ok(Self { path, contents })
    }
}

impl Drop for HostRuntime {
    fn drop(&mut self) {
        if std::fs::read(&self.path).is_ok_and(|contents| contents == self.contents) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub fn read(repo: &Path, scope: &str) -> anyhow::Result<Option<RunningHost>> {
    let path = path(repo, scope);
    match std::fs::read(&path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .with_context(|| format!("review host runtime file is invalid: {path:?}"))
            .map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("cannot read {path:?}")),
    }
}

fn path(repo: &Path, scope: &str) -> PathBuf {
    repo.join(".provenance/cache/review-hosts")
        .join(format!("{scope}.json"))
}
