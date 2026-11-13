//! The git adapter over a Git history that holds the commits of a case.

use camino::Utf8PathBuf;
use provenance_store::review::history::contract::{Adapter, Commit, Harness};
use provenance_store::review::history::GitFileVersions;
use std::collections::BTreeMap;
use std::io::Write;
use std::process::Stdio;
use std::sync::Arc;

/// A Git repository that holds the commits, made with Git's plumbing so
/// that each commit has exactly the parents and files of the graph.
pub struct Git {
    temps: std::sync::Mutex<Vec<tempfile::TempDir>>,
}

fn git(
    root: &std::path::Path,
    env: &[(&str, String)],
    args: &[&str],
    input: Option<&[u8]>,
) -> String {
    let mut command = provenance_store::git_process::git();
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input.unwrap_or_default()).unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

impl Harness for Git {
    fn adapter(&self, commits: &[Commit]) -> Adapter {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        git(root, &[], &["init", "-q"], None);
        let mut ids: BTreeMap<String, String> = BTreeMap::new();
        for commit in commits {
            let base = commit
                .parents
                .first()
                .map_or("--empty", |first| ids[first.as_str()].as_str());
            git(root, &[], &["read-tree", base], None);
            for (path, entry) in &commit.changes {
                let Some(entry) = entry else {
                    git(root, &[], &["update-index", "--force-remove", path], None);
                    continue;
                };
                let input = Some(entry.bytes.as_slice());
                let blob = git(root, &[], &["hash-object", "-w", "--stdin"], input);
                let mode = if entry.executable { "100755" } else { "100644" };
                let entry = format!("{mode},{blob},{path}");
                git(
                    root,
                    &[],
                    &["update-index", "--add", "--cacheinfo", &entry],
                    None,
                );
            }
            let tree = git(root, &[], &["write-tree"], None);
            let mut args = vec!["commit-tree".to_owned(), tree];
            for parent in &commit.parents {
                args.push("-p".to_owned());
                args.push(ids[parent.as_str()].clone());
            }
            args.extend(["-m".to_owned(), commit.name.clone()]);
            let env = [
                ("GIT_AUTHOR_NAME", commit.author.clone()),
                ("GIT_AUTHOR_EMAIL", format!("{}@example.com", commit.author)),
                ("GIT_COMMITTER_NAME", commit.committer.clone()),
                (
                    "GIT_COMMITTER_EMAIL",
                    format!("{}@example.com", commit.committer),
                ),
                ("GIT_AUTHOR_DATE", format!("{} +0000", commit.authored_at)),
                (
                    "GIT_COMMITTER_DATE",
                    format!("{} +0000", commit.committed_at),
                ),
            ];
            let args = args.iter().map(String::as_str).collect::<Vec<_>>();
            ids.insert(commit.name.clone(), git(root, &env, &args, None));
        }
        let head = &ids[commits.last().unwrap().name.as_str()];
        git(root, &[], &["update-ref", "refs/heads/main", head], None);
        git(
            root,
            &[],
            &["symbolic-ref", "HEAD", "refs/heads/main"],
            None,
        );
        let root = Utf8PathBuf::from_path_buf(dunce::canonicalize(root).unwrap()).unwrap();
        self.temps.lock().unwrap().push(temp);
        Adapter::new(Arc::new(GitFileVersions::new(root)), ids)
    }
}

pub const fn git_harness() -> Git {
    Git {
        temps: std::sync::Mutex::new(Vec::new()),
    }
}
