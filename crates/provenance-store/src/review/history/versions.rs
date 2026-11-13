//! The file versions port: the committed versions of one repository file.
//!
//! Record history reads the versions of a graph record file and the
//! Discussion file at those commits through this port. The git adapter
//! reads them from the Git history of the working tree. An embedder whose
//! history is somewhere else gives its own adapter to
//! [`Repository::with_file_versions`](crate::repository::Repository::with_file_versions).
//!
//! A path names a file below the working tree of the repository, with `/`
//! between its parts, such as `.provenance/state/manifest.json`.

use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// The future of one port call.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One commit that changed a file, with the file text at that commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileVersion {
    pub commit: String,
    /// The nearest listed commits on each parent line, in parent order. The
    /// file text at each is the file text at the matching parent. A root
    /// commit, or a commit at the edge of a shallow clone, has none.
    pub parents: Vec<String>,
    pub author: String,
    /// The commit time in seconds since the Unix epoch.
    pub committed_at: i64,
    /// `None` when the commit removed the file.
    pub text: Option<String>,
}

/// The committed versions of the files of one repository.
pub trait FileVersions: Send + Sync + Debug {
    /// The commits in the history of `HEAD` that changed `path`, each
    /// parent before its children, with the version of `HEAD` last.
    ///
    /// A merge whose file is the same as the file of one parent is not
    /// listed, and the history goes on through that parent only. A merge
    /// whose file differs from the file of each parent is listed. A history
    /// with no commit gives no versions.
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>>;

    /// The text of `path` at each commit of `commits`, in the same order.
    ///
    /// The commit need not have changed the file. `None` when the file is
    /// not in that commit.
    fn texts<'a>(
        &'a self,
        path: &'a str,
        commits: &'a [String],
    ) -> BoxFuture<'a, anyhow::Result<Vec<Option<String>>>>;
}
