//! Init sends the state files of a repository to the `provenance-jsonl`
//! merge driver and sets the command of that driver in the clone, so a real
//! `git merge` of two branches combines their records.

use provenance_macros::verifies;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DRIVER_KEY: &str = "merge.provenance-jsonl.driver";
const NATIVE_DRIVER: &str = "provenance merge-jsonl %O %A %B --output %A --path %P";
const STATE_MERGE_LINE: &str = ".provenance/state/**/*.jsonl merge=provenance-jsonl";

/// A folder with an empty global git configuration, no system
/// configuration, and the built binary first on `PATH`, so git finds the
/// `provenance` command that init writes.
struct Host {
    directory: tempfile::TempDir,
}

impl Host {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("gitconfig"), "").unwrap();
        Self { directory }
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn with_environment(&self, command: &mut Command) {
        let binary = PathBuf::from(assert_cmd::cargo::cargo_bin!("provenance"));
        let mut path = vec![binary.parent().unwrap().to_path_buf()];
        path.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        command
            .env("PATH", std::env::join_paths(path).unwrap())
            .env("GIT_CONFIG_GLOBAL", self.root().join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.test")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.test")
            .env("PROVENANCE_STE100_ASSET_DIR", self.root().join("assets"))
            .env("PROVENANCE_STE100_INDEX_DIR", self.root().join("indexes"))
            .env(
                "PROVENANCE_TEST_STE100_ASSET_URL",
                env!("PROVENANCE_TEST_STE100_ASSET_URL"),
            );
    }

    fn git(&self, folder: &Path, args: &[&str]) -> Output {
        let mut command = provenance_store::git_process::git();
        self.with_environment(&mut command);
        command.current_dir(folder).args(args).output().unwrap()
    }

    fn git_ok(&self, folder: &Path, args: &[&str]) -> String {
        let output = self.git(folder, args);
        assert!(
            output.status.success(),
            "git {args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn provenance(&self, args: &[&str]) -> Output {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
        self.with_environment(&mut command);
        command.args(args).output().unwrap()
    }

    /// Runs init of `folder` in the scope "default" and returns its output.
    fn init(&self, folder: &Path, extra: &[&str]) -> (String, String) {
        let mut args = vec!["init", "--path", folder.to_str().unwrap()];
        args.extend(["--scope", "default", "--path-prefix", "."]);
        args.extend(extra);
        let output = self.provenance(&args);
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{stdout}{stderr}");
        (stdout, stderr)
    }

    /// A new git repository below the host folder.
    fn repository(&self, name: &str) -> PathBuf {
        let repo = self.root().join(name);
        std::fs::create_dir(&repo).unwrap();
        self.git_ok(&repo, &["init", "--quiet", "--initial-branch", "main"]);
        repo
    }

    fn local_driver(&self, repo: &Path) -> Option<String> {
        let output = self.git(repo, &["config", "--local", "--get", DRIVER_KEY]);
        output.status.success().then(|| {
            String::from_utf8(output.stdout)
                .unwrap()
                .trim_end()
                .to_owned()
        })
    }

    fn create_requirement(&self, repo: &Path, id: &str) {
        let output = self.provenance(&[
            id,
            "create",
            "--type",
            "requirement",
            "--statement",
            "The host starts.",
            "--repo",
            repo.to_str().unwrap(),
        ]);
        assert!(output.status.success(), "{output:?}");
    }

    fn commit_all(&self, repo: &Path, message: &str) {
        self.git_ok(repo, &["add", "."]);
        self.git_ok(repo, &["commit", "--quiet", "-m", message]);
    }

    /// The number of Requirements with each id in `ids` that a read returns.
    fn requirement_counts(&self, repo: &Path, ids: &[&str]) -> Vec<usize> {
        let output = self.provenance(&[
            "api",
            "/requirements",
            "--format",
            "json",
            "--repo",
            repo.to_str().unwrap(),
        ]);
        assert!(output.status.success(), "{output:?}");
        let body: Value = serde_json::from_slice(&output.stdout).unwrap();
        let text = body.to_string();
        ids.iter()
            .map(|id| text.matches(&format!("\"id\":\"{id}\"")).count())
            .collect()
    }
}

#[test]
#[verifies("rule_init_routes_state_merges", examples)]
#[verifies("rule_init_sets_clone_merge_driver", examples)]
fn a_merge_of_two_branches_after_init_keeps_the_records_of_both() {
    let host = Host::new();
    let repo = host.repository("repo");

    let (summary, _) = host.init(&repo, &[]);

    assert!(summary.contains(".gitattributes"), "{summary}");
    assert!(summary.contains("git config of this clone"), "{summary}");
    assert_eq!(
        std::fs::read_to_string(repo.join(".gitattributes")).unwrap(),
        format!("{STATE_MERGE_LINE}\n")
    );
    assert_eq!(host.local_driver(&repo).as_deref(), Some(NATIVE_DRIVER));
    host.create_requirement(&repo, "req_base");
    host.commit_all(&repo, "base");
    host.git_ok(&repo, &["checkout", "--quiet", "-b", "theirs"]);
    host.create_requirement(&repo, "req_theirs");
    host.commit_all(&repo, "theirs");
    host.git_ok(&repo, &["checkout", "--quiet", "main"]);
    host.create_requirement(&repo, "req_ours");
    host.commit_all(&repo, "ours");

    host.git_ok(&repo, &["merge", "--quiet", "theirs", "-m", "merge"]);

    assert_eq!(
        host.requirement_counts(&repo, &["req_base", "req_ours", "req_theirs"]),
        [1, 1, 1]
    );
}

#[test]
#[verifies("rule_init_routes_state_merges", examples)]
#[verifies("rule_init_sets_clone_merge_driver", examples)]
fn a_second_init_changes_nothing() {
    let host = Host::new();
    let repo = host.repository("repo");
    host.init(&repo, &[]);

    let (summary, _) = host.init(&repo, &[]);

    assert!(summary.contains("No change."), "{summary}");
    assert_eq!(
        std::fs::read_to_string(repo.join(".gitattributes")).unwrap(),
        format!("{STATE_MERGE_LINE}\n")
    );
}

#[test]
#[verifies("rule_init_sets_clone_merge_driver", examples)]
fn init_in_a_fresh_clone_sets_only_the_driver() {
    let host = Host::new();
    let origin = host.repository("origin");
    host.init(&origin, &[]);
    host.commit_all(&origin, "init");
    host.git_ok(host.root(), &["clone", "--quiet", "origin", "clone"]);
    let clone = host.root().join("clone");
    assert_eq!(host.local_driver(&clone), None);

    let (summary, _) = host.init(&clone, &[]);

    assert!(summary.contains("git config of this clone"), "{summary}");
    for file in [".gitattributes", ".gitignore", "AGENTS.md", "manifest.json"] {
        assert!(!summary.contains(file), "{file} in {summary}");
    }
    assert_eq!(host.local_driver(&clone).as_deref(), Some(NATIVE_DRIVER));
    assert_eq!(host.git_ok(&clone, &["status", "--porcelain"]), "");
}

#[test]
#[verifies("rule_init_sets_clone_merge_driver", examples)]
fn a_driver_value_of_the_global_configuration_stays() {
    let host = Host::new();
    let global = "/opt/provenance/bin/provenance merge-jsonl %O %A %B --output %A --path %P";
    std::fs::write(
        host.root().join("gitconfig"),
        format!("[merge \"provenance-jsonl\"]\n\tdriver = {global}\n"),
    )
    .unwrap();
    let repo = host.repository("repo");

    let (summary, stderr) = host.init(&repo, &[]);

    assert!(!summary.contains("git config"), "{summary}");
    assert!(!stderr.contains("merge driver"), "{stderr}");
    assert_eq!(host.local_driver(&repo), None);
    assert_eq!(
        host.git_ok(&repo, &["config", "--get", DRIVER_KEY])
            .trim_end(),
        global
    );
}

#[test]
#[verifies("rule_init_routes_state_merges", examples)]
#[verifies("rule_init_sets_clone_merge_driver", examples)]
fn init_outside_a_repository_writes_the_line_and_gives_a_notice() {
    let host = Host::new();
    let folder = host.root().join("project");
    std::fs::create_dir(&folder).unwrap();

    let (summary, stderr) = host.init(&folder, &[]);

    assert!(summary.contains(".gitattributes"), "{summary}");
    assert!(!summary.contains("git config"), "{summary}");
    assert!(
        stderr.contains("This folder is not in a git work tree, so init did not set the merge driver for state files."),
        "{stderr}"
    );
    assert!(folder.join(".gitattributes").is_file());
}

#[test]
#[verifies("rule_init_sets_clone_merge_driver", examples)]
fn a_typescript_project_below_the_top_of_the_work_tree_gets_the_command() {
    let host = Host::new();
    let repo = host.repository("repo");
    let project = repo.join("web");
    std::fs::create_dir(&project).unwrap();

    let (summary, stderr) = host.init(
        &project,
        &[
            "--invocation-channel",
            "typescript",
            "--package-manager",
            "npm",
        ],
    );

    assert!(!summary.contains("git config"), "{summary}");
    assert!(
        stderr.contains(&format!(
            "git config --local {DRIVER_KEY} 'node node_modules/@quality-sh/provenance/bin/provenance.mjs merge-jsonl %O %A %B --output %A --path %P'"
        )),
        "{stderr}"
    );
    assert_eq!(host.local_driver(&repo), None);
}

#[test]
#[verifies("rule_init_sets_clone_merge_driver", examples)]
fn a_typescript_project_at_the_top_of_the_work_tree_gets_its_command() {
    let host = Host::new();
    let repo = host.repository("repo");

    host.init(
        &repo,
        &[
            "--invocation-channel",
            "typescript",
            "--package-manager",
            "npm",
        ],
    );

    assert_eq!(
        host.local_driver(&repo).as_deref(),
        Some("node node_modules/@quality-sh/provenance/bin/provenance.mjs merge-jsonl %O %A %B --output %A --path %P")
    );
}
