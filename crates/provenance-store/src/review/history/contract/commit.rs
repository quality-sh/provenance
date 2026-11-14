//! One commit of a commit graph of the contract, and one file of a commit.

/// A file that one commit holds: its bytes and its mode.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Entry {
    pub bytes: Vec<u8>,
    /// The file has the executable mode.
    pub executable: bool,
}

impl Entry {
    /// A file with `bytes`, executable when `executable` is set.
    pub const fn new(bytes: Vec<u8>, executable: bool) -> Self {
        Self { bytes, executable }
    }

    /// A file that is not executable, with `text` as its bytes.
    pub fn text(text: &str) -> Self {
        Self::new(text.as_bytes().to_vec(), false)
    }
}

/// One commit of a commit graph.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Commit {
    pub name: String,
    /// The names of the parents, in parent order.
    pub parents: Vec<String>,
    /// The author name.
    pub author: String,
    /// The committer name.
    pub committer: String,
    /// The author time in seconds since the Unix epoch.
    pub authored_at: i64,
    /// The commit time in seconds since the Unix epoch.
    pub committed_at: i64,
    /// The files that differ from the first parent: the new file, or `None`
    /// for a removed file. A merge has the files of its first parent and
    /// these changes.
    pub changes: Vec<(String, Option<Entry>)>,
}

impl Commit {
    /// A commit with these parents, in parent order, that changes no file.
    /// Its author is also its committer, and its author time is its commit
    /// time.
    pub fn new(name: &str, parents: &[&str], author: &str, committed_at: i64) -> Self {
        Self {
            name: name.to_owned(),
            parents: parents.iter().map(|parent| (*parent).to_owned()).collect(),
            author: author.to_owned(),
            committer: author.to_owned(),
            authored_at: committed_at,
            committed_at,
            changes: Vec::new(),
        }
    }

    /// This commit with `committer` as the committer name and `authored_at`
    /// as the author time, as a commit that another person rebased has.
    #[must_use]
    pub fn with_committer(mut self, committer: &str, authored_at: i64) -> Self {
        committer.clone_into(&mut self.committer);
        self.authored_at = authored_at;
        self
    }

    /// This commit with `text` as the file at `path`, not executable.
    #[must_use]
    pub fn with_file(self, path: &str, text: &str) -> Self {
        self.with_entry(path, Entry::text(text))
    }

    /// This commit with `entry` as the file at `path`.
    #[must_use]
    pub fn with_entry(mut self, path: &str, entry: Entry) -> Self {
        self.changes.push((path.to_owned(), Some(entry)));
        self
    }

    /// This commit with no file at `path`.
    #[must_use]
    pub fn without_file(mut self, path: &str) -> Self {
        self.changes.push((path.to_owned(), None));
        self
    }
}
