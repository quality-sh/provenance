use camino::{Utf8Path, Utf8PathBuf};

#[derive(Debug, Clone)]
pub struct ProvenanceLayout {
    root: Utf8PathBuf,
}

impl ProvenanceLayout {
    pub fn new(root: impl Into<Utf8PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn provenance_dir(&self) -> Utf8PathBuf {
        self.root.join(".provenance")
    }
    pub(crate) fn root(&self) -> &Utf8Path {
        &self.root
    }
    pub fn state_dir(&self) -> Utf8PathBuf {
        self.provenance_dir().join("state")
    }
    pub fn manifest_path(&self) -> Utf8PathBuf {
        self.state_dir().join("manifest.json")
    }
    pub fn scopes_dir(&self) -> Utf8PathBuf {
        self.state_dir().join("scopes")
    }
    pub fn edges_dir(&self) -> Utf8PathBuf {
        self.state_dir().join("edges")
    }
    pub fn cache_dir(&self) -> Utf8PathBuf {
        self.provenance_dir().join("cache")
    }
    /// Default output directory for the generated wiki: a derived artifact,
    /// gitignored the same way `cache_dir` is.
    pub fn wiki_dir(&self) -> Utf8PathBuf {
        self.provenance_dir().join("wiki")
    }
    pub fn cache_db_path(&self) -> Utf8PathBuf {
        self.cache_dir().join("provenance.db")
    }
    pub fn verification_runs_path(&self, scope: &provenance_core::ScopeId) -> Utf8PathBuf {
        self.cache_dir()
            .join("scopes")
            .join(scope.as_str())
            .join("verification-runs.jsonl")
    }
    pub fn verification_runs_lock_path(&self, scope: &provenance_core::ScopeId) -> Utf8PathBuf {
        self.cache_dir()
            .join("locks/scopes")
            .join(scope.as_str())
            .join("verification-runs.jsonl.lock")
    }
    pub fn state_shard_lock_path(&self, shard_path: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
        let relative = shard_path.strip_prefix(self.state_dir()).map_err(|_| {
            anyhow::anyhow!("state shard path {shard_path} is outside the state directory")
        })?;
        Ok(self
            .cache_dir()
            .join("locks")
            .join(relative)
            .with_extension("jsonl.lock"))
    }
    pub fn lifecycle_lock_path(&self, scope: &provenance_core::ScopeId) -> Utf8PathBuf {
        self.cache_dir()
            .join("locks/scopes")
            .join(scope.as_str())
            .join("ideation.lifecycle.lock")
    }
    pub fn publication_lock_path(&self) -> Utf8PathBuf {
        self.cache_dir().join("locks/repository.publication.lock")
    }
    /// Directory holding the projection write journal: the event tail and
    /// the sequence head record.
    pub fn journal_dir(&self) -> Utf8PathBuf {
        self.cache_dir().join("journal")
    }
    pub fn journal_events_path(&self) -> Utf8PathBuf {
        self.journal_dir().join("events.jsonl")
    }
    pub fn journal_head_path(&self) -> Utf8PathBuf {
        self.journal_dir().join("head.json")
    }
    pub fn publication_marker_path(&self) -> Utf8PathBuf {
        self.cache_dir().join("import-publication.json")
    }
    pub fn import_transactions_dir(&self) -> Utf8PathBuf {
        self.cache_dir().join("import-transactions")
    }
}

pub fn locate_repo_root(start: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join(".git").exists()
            || candidate.join(".provenance/state/manifest.json").exists()
        {
            return Ok(candidate.to_path_buf());
        }
    }
    anyhow::bail!("could not locate repository root from {start}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_paths_are_under_provenance_directory() {
        let root = Utf8PathBuf::from("repo");
        let layout = ProvenanceLayout::new(root.clone());
        assert_eq!(
            layout.manifest_path(),
            root.join(".provenance/state/manifest.json")
        );
        assert_eq!(
            layout.cache_db_path(),
            root.join(".provenance/cache/provenance.db")
        );
        assert_eq!(layout.wiki_dir(), root.join(".provenance/wiki"));
        assert_eq!(
            layout
                .state_shard_lock_path(&layout.scopes_dir().join("default/sources/source.jsonl"))
                .unwrap(),
            root.join(".provenance/cache/locks/scopes/default/sources/source.jsonl.lock")
        );
    }
}
