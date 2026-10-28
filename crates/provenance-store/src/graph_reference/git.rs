use super::{incomplete, GraphReferenceError, STORE_PATH};
use camino::{Utf8Path, Utf8PathBuf};
use std::process::Command;

#[derive(Clone, Copy)]
pub(super) enum TreeSource<'a> {
    Commit(&'a str),
    Index,
}

pub(super) struct GitRepository {
    root: Utf8PathBuf,
}

impl GitRepository {
    pub(super) fn open(path: &Utf8Path) -> Result<Self, GraphReferenceError> {
        let output = run_git(path, &["rev-parse", "--show-toplevel"])?;
        let root = String::from_utf8(output).map_err(|error| GraphReferenceError::Incomplete {
            detail: format!("Git repository path is not UTF-8: {error}"),
        })?;
        let root = Utf8PathBuf::from(root.trim());
        if root.as_str().is_empty() {
            return Err(GraphReferenceError::Missing {
                detail: format!("{} is not a Git repository", path.as_str()),
            });
        }
        let shallow = run_git(&root, &["rev-parse", "--is-shallow-repository"])?;
        if shallow == b"true\n" {
            return Err(GraphReferenceError::Incomplete {
                detail: "graph references require complete Git history; unshallow this repository"
                    .into(),
            });
        }
        Ok(Self { root })
    }

    pub(super) fn root(&self) -> &Utf8Path {
        &self.root
    }

    pub(super) fn resolve_commit(&self, revision: &str) -> Result<String, GraphReferenceError> {
        let spec = format!("{revision}^{{commit}}");
        let output = run_git(&self.root, &["rev-parse", "--verify", &spec]).map_err(|_| {
            GraphReferenceError::Missing {
                detail: format!("Git revision '{revision}' does not resolve to a commit"),
            }
        })?;
        String::from_utf8(output)
            .map(|value| value.trim().to_string())
            .map_err(|error| GraphReferenceError::Incomplete {
                detail: format!("Git commit ID is not UTF-8: {error}"),
            })
    }

    pub(super) fn identity(&self, commit: &str) -> Result<String, GraphReferenceError> {
        let output = run_git(&self.root, &["rev-list", "--max-parents=0", commit])?;
        let roots = String::from_utf8(output).map_err(|error| GraphReferenceError::Incomplete {
            detail: format!("Git root commit IDs are not UTF-8: {error}"),
        })?;
        let mut roots: Vec<_> = roots.lines().collect();
        roots.sort_unstable();
        if roots.is_empty() {
            return Err(GraphReferenceError::Incomplete {
                detail: format!("commit {commit} has no repository root"),
            });
        }
        let identity = format!("git-repository-v1\0{}\0{STORE_PATH}", roots.join("\0"));
        Ok(format!(
            "git1_{}",
            crate::canonical_digest::sha256(identity.as_bytes())
        ))
    }

    pub(super) fn materialize(
        &self,
        source: TreeSource<'_>,
    ) -> Result<tempfile::TempDir, GraphReferenceError> {
        let temp = tempfile::tempdir().map_err(incomplete)?;
        let (paths, commit) = match source {
            TreeSource::Commit(commit) => (
                run_git(
                    &self.root,
                    &[
                        "ls-tree",
                        "-r",
                        "-z",
                        "--name-only",
                        commit,
                        "--",
                        STORE_PATH,
                    ],
                )?,
                Some(commit),
            ),
            TreeSource::Index => (
                run_git(&self.root, &["ls-files", "-z", "--", STORE_PATH])?,
                None,
            ),
        };
        for raw_path in paths
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let path = std::str::from_utf8(raw_path).map_err(incomplete)?;
            let relative = std::path::Path::new(path);
            if !path.starts_with(&format!("{STORE_PATH}/"))
                || relative
                    .components()
                    .any(|component| !matches!(component, std::path::Component::Normal(_)))
            {
                return Err(GraphReferenceError::Incomplete {
                    detail: format!("Git returned unsafe canonical path '{path}'"),
                });
            }
            let spec =
                commit.map_or_else(|| format!(":{path}"), |commit| format!("{commit}:{path}"));
            let contents = run_git(&self.root, &["show", &spec])?;
            let destination = temp.path().join(path);
            let parent = destination
                .parent()
                .ok_or_else(|| GraphReferenceError::Incomplete {
                    detail: format!("canonical path '{path}' has no parent"),
                })?;
            std::fs::create_dir_all(parent).map_err(incomplete)?;
            std::fs::write(destination, contents).map_err(incomplete)?;
        }
        Ok(temp)
    }
}

fn run_git(repo: &Utf8Path, args: &[&str]) -> Result<Vec<u8>, GraphReferenceError> {
    let output = Command::new("git")
        .arg("--no-replace-objects")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| GraphReferenceError::Missing {
            detail: format!("could not execute Git: {error}"),
        })?;
    if !output.status.success() {
        return Err(GraphReferenceError::Missing {
            detail: format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(output.stdout)
}
