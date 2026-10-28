//! The evidence at the base commit that the report compares with. The base
//! source tree is read once: the evidence comparison scans it, and the site
//! comparison reads the same files.

use crate::envelope::CommitRole;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ImplementationBinding, ScopeId, VerificationBinding};
use provenance_scanner::{FileScan, Language};
use provenance_store::stale::git::{self, RevisionFile};

use super::graph_snapshots;

/// The evidence read at the base commit. A part that cannot be read stays
/// empty, and `failure` states why.
#[derive(Default)]
pub(super) struct BaseEvidence {
    /// The base source files. `None` when they cannot be read; the site
    /// comparison then has no base.
    pub(super) files: Option<Vec<RevisionFile>>,
    pub(super) scans: Vec<FileScan>,
    pub(super) verifications: Vec<VerificationBinding>,
    pub(super) implementations: Vec<ImplementationBinding>,
    pub(super) failure: Option<BaseEvidenceError>,
}

/// Why the evidence at the base commit cannot be read.
pub(super) enum BaseEvidenceError {
    SourceFiles(anyhow::Error),
    UnsupportedLanguage(Utf8PathBuf),
    Bindings(anyhow::Error),
}

impl BaseEvidenceError {
    /// The baseline reason the envelope states.
    pub(super) fn reason(&self, base: &str) -> String {
        match self {
            Self::SourceFiles(error) => {
                format!("source files at base commit {base} cannot be read: {error:#}")
            }
            Self::UnsupportedLanguage(path) => format!(
                "source files at base commit {base} cannot be read: {path} has no supported language"
            ),
            Self::Bindings(error) => {
                format!("binding records at base commit {base} do not parse: {error:#}")
            }
        }
    }
}

/// Read the base source files, scan them, and read the base binding
/// records.
pub(super) fn read(repo: &Utf8Path, base: &str, scope: &ScopeId) -> BaseEvidence {
    let files = match git::revision_files(repo, base) {
        Ok(files) => files,
        Err(error) => return failed(BaseEvidenceError::SourceFiles(error)),
    };
    let scans = match scan(&files) {
        Ok(scans) => scans,
        Err(error) => return failed(error),
    };
    match graph_snapshots::read_bindings(repo, base, scope, CommitRole::Base) {
        Ok((verifications, implementations)) => BaseEvidence {
            files: Some(files),
            scans,
            verifications,
            implementations,
            failure: None,
        },
        // The source files stay readable, so the site comparison keeps them.
        Err(error) => BaseEvidence {
            files: Some(files),
            failure: Some(BaseEvidenceError::Bindings(error)),
            ..BaseEvidence::default()
        },
    }
}

fn failed(error: BaseEvidenceError) -> BaseEvidence {
    BaseEvidence {
        failure: Some(error),
        ..BaseEvidence::default()
    }
}

fn scan(files: &[RevisionFile]) -> Result<Vec<FileScan>, BaseEvidenceError> {
    files
        .iter()
        .map(|file| {
            let language = file
                .path
                .extension()
                .and_then(Language::from_extension)
                .ok_or_else(|| BaseEvidenceError::UnsupportedLanguage(file.path.clone()))?;
            Ok(provenance_scanner::scan_file(
                &file.path,
                language,
                &file.content,
            ))
        })
        .collect()
}
