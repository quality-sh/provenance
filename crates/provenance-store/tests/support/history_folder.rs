//! The history of `history_fixture.rs` as a folder repository with real Git
//! history: each commit made with Git's plumbing, so it has exactly the
//! parents, files, authors and times of the graph, then the work tree at the
//! last commit and the saved working copy over it.

use super::history_fixture::History;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::process::Stdio;

/// A folder repository and the name of each of its commits by commit id.
pub struct Folder {
    pub temp: tempfile::TempDir,
    pub names: BTreeMap<String, String>,
}

impl Folder {
    pub fn root(&self) -> &Path {
        self.temp.path()
    }

    /// `text` with each commit id as its name in the graph.
    pub fn named(&self, text: &str) -> String {
        self.names
            .iter()
            .fold(text.to_owned(), |text, (id, name)| text.replace(id, name))
    }
}

fn git(root: &Path, env: &[(&str, String)], args: &[&str], input: Option<&[u8]>) -> String {
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

/// Makes the folder repository of `history`.
pub fn folder(history: &History) -> Folder {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    git(root, &[], &["init", "-q"], None);
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    for commit in &history.commits {
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
            let blob = git(
                root,
                &[],
                &["hash-object", "-w", "--stdin"],
                Some(&entry.bytes),
            );
            let entry = format!("100644,{blob},{path}");
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
    let head = &ids[history.commits.last().unwrap().name.as_str()];
    git(root, &[], &["update-ref", "refs/heads/main", head], None);
    git(
        root,
        &[],
        &["symbolic-ref", "HEAD", "refs/heads/main"],
        None,
    );
    git(root, &[], &["read-tree", "HEAD"], None);
    git(root, &[], &["checkout-index", "-a", "-f"], None);
    for (path, bytes) in &history.working {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    Folder {
        temp,
        names: ids.into_iter().map(|(name, id)| (id, name)).collect(),
    }
}
