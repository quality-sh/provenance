//! The `git cat-file` process of the git adapter, seen through a `git` that
//! logs it.
//!
//! A `git` first on PATH writes its arguments to `.git/git-starts` of the
//! repository, then runs the real Git. When `.git/cat-file-answer` is there,
//! a `cat-file` process reads one request and gives that file as its answer.
//! The process list cannot show a process that started and stopped between
//! two looks, and one Git gives only one of the answers that Git versions
//! give for a submodule entry. The script is a shell script, so the tests are
//! Unix-only. `history_git_children` tests the life of the process on each
//! platform.
//!
//! No Rule states when the git adapter starts a Git process, or how it reads
//! an answer that ends before its object, a `missing` answer or an answer of
//! an unknown form, so those tests are implementation aids with no Rule
//! binding. The reads of a record file with more than 1,000 versions are
//! evidence of `rule_history_reads_bound_file_versions` and
//! `rule_record_history_reads_index`.
#![cfg(unix)]

mod discussion_support;
#[path = "history_git_support/repository.rs"]
mod repository;

use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::review::ReviewHistoryQuery;
use provenance_core::NodeType;
use provenance_macros::verifies;
use provenance_store::operations::read_operations::{InProcessReadOperations, ReadOperations};
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::operations::LocalRepository;
use provenance_store::review::history::contract::UNKNOWN_COMMIT;
use provenance_store::review::history::{FileVersions, GitFileVersions};
use provenance_store::review::read_history;
use repository::repository;
use serde_json::json;
use std::fmt::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

/// Puts the logging `git` first on PATH, once for the test binary, before
/// any test starts Git.
fn logging_git() {
    static TOOLS: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    TOOLS.get_or_init(|| {
        let real = std::process::Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap();
        let real = String::from_utf8(real.stdout).unwrap();
        let real = real.trim();
        let tools = tempfile::tempdir().unwrap();
        let wrapper = tools.path().join("git");
        std::fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\necho \"$*\" 2>/dev/null >> .git/git-starts\n\
                 case \" $* \" in *' cat-file '*)\n\
                 if [ -f .git/cat-file-answer ]; then\n\
                 read -r name; cat .git/cat-file-answer; exit 0; fi;;\n\
                 esac\nexec '{real}' \"$@\"\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(tools.path().to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .unwrap();
        std::env::set_var("PATH", path);
        tools
    });
}

/// The number of Git processes that started in the repository at `root`
/// and ran to their first command. A process that the adapter stops at once
/// can stop before that.
fn starts(root: &Path) -> usize {
    std::fs::read_to_string(root.join(".git/git-starts")).map_or(0, |starts| starts.lines().count())
}

#[tokio::test]
async fn a_refused_name_starts_no_process() {
    logging_git();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    let before = starts(temp.path());
    let versions = GitFileVersions::new(root);
    for path in ["go\nod.jsonl", "good.jsonl\r"] {
        assert!(versions.text(path, &commit).await.is_err(), "{path:?}");
    }
    assert_eq!(starts(temp.path()), before, "a refused name started Git");
    let text = versions.text("good.jsonl", &commit).await.unwrap();
    assert_eq!(text.as_deref(), Some("good\n"));
    assert!(starts(temp.path()) > before, "the log has no Git start");
}

#[tokio::test]
async fn an_answer_that_ends_before_its_object_is_refused() {
    logging_git();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    std::fs::write(temp.path().join(".git/cat-file-answer"), "x blob 10\nabc").unwrap();
    let versions = GitFileVersions::new(root);
    let error = versions.text("good.jsonl", &commit).await.unwrap_err();
    assert_eq!(format!("{error:#}"), "git cat-file output ended early");
}

/// The text of `good.jsonl` when `cat-file` gives `answer`, made from the
/// commit id, to the request.
async fn text_for_answer(answer: fn(&str) -> String) -> anyhow::Result<Option<String>> {
    logging_git();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    std::fs::write(temp.path().join(".git/cat-file-answer"), answer(&commit)).unwrap();
    GitFileVersions::new(root).text("good.jsonl", &commit).await
}

/// Git 2.51 and later answer a submodule entry to a commit that the
/// repository does not hold with that commit and `submodule`.
#[tokio::test]
#[verifies("rule_record_history_text_only_from_file", examples)]
async fn a_submodule_entry_that_git_answers_as_a_submodule_has_no_text() {
    let text = text_for_answer(|_| format!("{UNKNOWN_COMMIT} submodule\n")).await;
    assert_eq!(text.unwrap(), None);
}

/// Pins the answer that Git before 2.51 gives for a submodule entry to a
/// commit that the repository does not hold: the name of the request and
/// `missing`, the answer for a name with no object.
#[tokio::test]
async fn a_missing_answer_has_no_text() {
    let text = text_for_answer(|commit| format!("{commit}:good.jsonl missing\n")).await;
    assert_eq!(text.unwrap(), None);
}

/// An answer of a form that the adapter does not know is refused, and the
/// error gives the answer.
#[tokio::test]
async fn an_answer_of_an_unknown_form_is_refused() {
    let error = text_for_answer(|_| "x tree\n".to_owned())
        .await
        .unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        "git cat-file returned an unreadable header: x tree"
    );
}

/// Adds `count` commits to the branch of `HEAD` with `git fast-import`, each
/// the child of the one before, that each give `req_a` in the Requirement
/// file a new statement. Then sets the work tree to the new `HEAD`.
fn statement_commits(temp: &tempfile::TempDir, count: usize) {
    let file = discussion_support::requirements(temp);
    let path = file
        .strip_prefix(camino::Utf8Path::from_path(temp.path()).unwrap())
        .unwrap()
        .as_str()
        .replace('\\', "/");
    let text = std::fs::read_to_string(&file).unwrap();
    let branch = discussion_support::git(temp, &["symbolic-ref", "HEAD"]);
    let head = discussion_support::git(temp, &["rev-parse", "HEAD"]);
    let mut stream = Vec::new();
    for index in 1..=count {
        let mut lines = String::new();
        for line in text.lines() {
            let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
            if record["id"] == "req_a" {
                record["statement"] = format!("Edit {index} of the statement.").into();
            }
            writeln!(lines, "{record}").unwrap();
        }
        let from = if index == 1 {
            head.clone()
        } else {
            format!(":{}", index - 1)
        };
        stream.extend(
            format!(
                "commit {branch}\nmark :{index}\ncommitter Ann <ann@example.com> {} +0000\n\
                 data 4\nedit\nfrom {from}\nM 100644 inline {path}\ndata {}\n{lines}\n",
                1_700_000_000 + index,
                lines.len()
            )
            .into_bytes(),
        );
    }
    let mut child = provenance_store::git_process::git()
        .args(["fast-import", "--quiet"])
        .current_dir(temp.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), &stream).unwrap();
    assert!(child.wait().unwrap().success());
    discussion_support::git(temp, &["reset", "-q", "--hard"]);
}

/// The history read of `req_a` in the repository at `temp`, opened again.
async fn read(temp: &tempfile::TempDir) -> anyhow::Result<Vec<String>> {
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: discussion_support::id(),
        limit: 200,
        cursor: None,
    };
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let page = read_history(
        &discussion_support::repository(root),
        ReadPolicy::default(),
        query,
    )
    .await?;
    Ok(page
        .result
        .entries
        .into_iter()
        .map(|entry| entry.id.as_str().to_owned())
        .collect())
}

/// The `cat-file` processes that started in the repository at `temp`.
fn cat_files(temp: &tempfile::TempDir) -> usize {
    let starts = std::fs::read_to_string(temp.path().join(".git/git-starts")).unwrap();
    starts
        .lines()
        .filter(|line| line.contains("cat-file"))
        .count()
}

/// A history read of a Git repository whose record file has more than 1,000
/// versions to index gives the budget refusal after it indexed 1,000, and
/// the next read answers. The index stays in the read database, so a read
/// in a repository opened again starts no `cat-file` process. After
/// `HEAD` moves back one commit, a read lists no commit past `HEAD`.
#[tokio::test]
#[verifies("rule_history_reads_bound_file_versions", examples)]
#[verifies("rule_record_history_reads_index", examples)]
async fn a_read_of_more_than_1000_file_versions_indexes_them_over_two_reads() {
    logging_git();
    let (temp, _store) = discussion_support::fixture();
    discussion_support::commit_state(&temp, "Create");
    statement_commits(&temp, 1_000);
    let error = read(&temp).await.expect_err("a refusal");
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded),
        "{error:#}"
    );
    let head = discussion_support::git(&temp, &["rev-parse", "HEAD"]);
    let previous = discussion_support::git(&temp, &["rev-parse", "HEAD~1"]);
    let first = read(&temp).await.unwrap();
    assert_eq!(first.len(), 200);
    let started = cat_files(&temp);
    assert_eq!(read(&temp).await.unwrap(), first);
    assert_eq!(cat_files(&temp), started, "a read of the indexed file");

    discussion_support::git(&temp, &["reset", "-q", "--hard", "HEAD~1"]);
    let entry = |id: &str| {
        let binding = LocalRepository::bind(
            discussion_support::repository(camino::Utf8Path::from_path(temp.path()).unwrap()),
            "folder",
        )
        .unwrap();
        let request = json!({
            "context": {"repository": "folder", "scope": "default"},
            "request": {"record_kind": "requirement", "record_id": "req_a", "entry_id": id},
        });
        async move {
            InProcessReadOperations::new(&binding)
                .run("review-history-entry", request)
                .await
                .map(|answer| answer["id"].clone())
                .map_err(|failure| failure.error)
        }
    };
    assert_eq!(entry(&previous).await, Ok(json!(previous)));
    assert_eq!(
        entry(&head).await,
        Err(json!({"kind": "resource_not_found"}))
    );
    assert_eq!(cat_files(&temp), started, "the commits are indexed");
}
