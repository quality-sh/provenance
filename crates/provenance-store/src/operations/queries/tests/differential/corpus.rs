//! The stores the differential harness runs over. Each corpus is one
//! temporary repository; no corpus is the workspace root, which would write
//! a database into the checkout and take the real publication lock.

use crate::cache::tests::fixtures;
use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::ScopeId;

pub struct Corpus {
    pub name: &'static str,
    _dir: tempfile::TempDir,
    pub root: Utf8PathBuf,
    pub scope: ScopeId,
    /// The first commit of a corpus with a git history, for the diff half
    /// of `evidence` and for `stale`.
    pub base_commit: Option<String>,
}

impl Corpus {
    fn new(name: &'static str, dir: tempfile::TempDir, scope: ScopeId) -> Self {
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        Self {
            name,
            _dir: dir,
            root,
            scope,
            base_commit: None,
        }
    }

    pub fn layout(&self) -> ProvenanceLayout {
        ProvenanceLayout::new(self.root.clone())
    }

    pub fn store(&self) -> StateStore {
        StateStore::new(self.layout())
    }
}

/// The seeded query store with a rule-bearing source file under two
/// commits, so the diff half of `evidence` and `stale` answer over it.
pub fn seeded_queries() -> Corpus {
    let (dir, _store, scope) = super::super::seeded_store();
    let mut corpus = Corpus::new("seeded_queries", dir, scope);
    let source = corpus.root.join("src/pay.rs");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, "#[rule(\"rule_overtime\")]\nfn pay() {}\n").unwrap();
    let base = git_commit(&corpus.root, "base");
    std::fs::write(
        &source,
        "#[rule(\"rule_overtime\")]\nfn pay() -> u32 {\n    1\n}\n",
    )
    .unwrap();
    git_commit(&corpus.root, "head");
    corpus.base_commit = base;
    corpus
}

/// The cache fixtures: the seeded layout with its binding and review
/// shards, and the owner-row layout with a row for every owner kind.
pub fn cache_fixtures() -> Vec<Corpus> {
    let (dir, layout, scope) = fixtures::seeded_layout();
    crate::cache::tests::projection_stamp_behavior::seed_integration_shards(
        &layout,
        scope.as_str(),
    );
    let seeded = Corpus::new("cache_seeded", dir, scope);
    let (dir, _layout, scope) = fixtures::owner_row_layout();
    let owners = Corpus::new("cache_owner_rows", dir, scope);
    vec![seeded, owners]
}

/// A copy of this repository's own canonical state.
pub fn repository_state() -> Corpus {
    let workspace = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize_utf8()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    copy_tree(
        &workspace.join(".provenance/state"),
        &ProvenanceLayout::new(root).state_dir(),
    );
    Corpus::new("repository_state", dir, ScopeId::new("default").unwrap())
}

fn copy_tree(source: &Utf8Path, destination: &Utf8Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in source.read_dir_utf8().unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Commits the tree and returns the commit id, or `None` when git is not
/// on the path; the corpus then runs without its diff cases.
fn git_commit(root: &Utf8Path, message: &str) -> Option<String> {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    if !root.join(".git").exists() {
        run(&["init", "-q"])?;
    }
    run(&["add", "-A"])?;
    run(&[
        "-c",
        "user.name=Provenance Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-q",
        "-m",
        message,
    ])?;
    run(&["rev-parse", "HEAD"])
}
