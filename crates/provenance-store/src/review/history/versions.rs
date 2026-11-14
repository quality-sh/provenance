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

/// One commit that changed a file. The port gives the text of the file at a
/// commit through [`FileVersions::text`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileVersion {
    pub commit: String,
    /// The nearest listed commits on each parent line, in parent order. The
    /// file text at each is the file text at the matching parent. A root
    /// commit, or a commit at the edge of a shallow clone, has none.
    pub parents: Vec<String>,
    /// The author name of the commit, not the committer name.
    pub author: String,
    /// The commit time (the committer date, not the author date) in seconds
    /// since the Unix epoch.
    pub committed_at: i64,
}

impl FileVersion {
    /// The version of a file at `commit`, whose nearest listed commits on
    /// each parent line are `parents`.
    pub const fn new(
        commit: String,
        parents: Vec<String>,
        author: String,
        committed_at: i64,
    ) -> Self {
        Self {
            commit,
            parents,
            author,
            committed_at,
        }
    }
}

/// The committed versions of the files of one repository.
pub trait FileVersions: Send + Sync + Debug {
    /// The commits in the history of `HEAD` that changed `path`, each
    /// parent before its children, each commit once.
    ///
    /// A commit changed the file when its file differs from the file of
    /// each of its parents, or when it is a root commit that holds the
    /// file. A removal is a change, and so is a change of the file mode
    /// only. A merge whose file is the same as the file of one parent is not
    /// listed, and the history goes on through
    /// that parent only, which can be any parent. When more than one parent
    /// has the same file, the history goes on through the first of them.
    /// The versions do not follow a moved file: a commit that moves a file
    /// to `path` is a change of `path` only, and the versions of the old
    /// path are not listed.
    ///
    /// The last version is the first commit that changed the file on the
    /// walk from `HEAD`: start at `HEAD` and go to the first parent with the
    /// same file until a commit changed the file. When `HEAD` did not change
    /// the file, `HEAD` is not listed. A history with no commit, or with no
    /// commit that changed the file, gives no versions.
    ///
    /// An adapter can give the same list, the same `Arc`, again while its
    /// history does not change. A history read keeps what it found for the
    /// last list that it read, so a new read of that list reads no text.
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>>;

    /// The text of `path` at `commit`.
    ///
    /// The commit need not have changed the file. `None` when the file is
    /// not in that commit, and when the adapter does not hold the commit. A
    /// name that is not a commit id of the adapter, such as an empty name or
    /// `HEAD`, names no commit that it holds. When the text is not UTF-8, the
    /// error is the [`FromUtf8Error`](std::string::FromUtf8Error) of its
    /// bytes.
    ///
    /// A history read asks for one text at a time and keeps only what it
    /// finds in it, so an adapter that reads each text on demand holds at
    /// most one text of the read.
    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>>;
}
