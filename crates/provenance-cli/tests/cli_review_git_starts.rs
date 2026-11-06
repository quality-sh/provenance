//! Implementation aid: no Rule states how often the review host starts Git. On Windows each
//! Git start from the detached host can show a console window, so these tests fix the Git
//! starts of each request step, and check the content that each step returns
//! (bead provenance-a5ru). The logging `git` is a shell script, so the tests are Unix-only.
#![cfg(unix)]

#[path = "review_host_support/mod.rs"]
mod review_host_support;

#[path = "cli_review_git_starts/support.rs"]
mod support;

use review_host_support::{
    follow::{create, pending_proposal},
    repository_with_actors,
};
use serde_json::json;
use support::{accept, comment, commit_all, edit, get, git, history, page_load, reviewer, GitLog};

const JANE: &str = "[user]\n\temail = jane@example.com\n";

/// One history entry of a commit by the test identity.
fn committed(commit: &str) -> (String, Option<String>) {
    (commit.to_owned(), Some("Test".to_owned()))
}

fn working() -> (String, Option<String>) {
    ("working".to_owned(), None)
}

/// Collects the Git starts of each step, so that one failure shows all steps.
struct Steps {
    log: GitLog,
    seen: Vec<(&'static str, Vec<String>)>,
}

impl Steps {
    fn record(&mut self, name: &'static str) {
        let starts = self.log.step();
        self.seen.push((name, starts));
    }
}

fn expected(steps: &[(&'static str, &[&str])]) -> Vec<(&'static str, Vec<String>)> {
    steps
        .iter()
        .map(|(name, starts)| (*name, starts.iter().map(|s| (*s).to_owned()).collect()))
        .collect()
}

#[test]
fn review_host_starts_git_once_per_value_that_can_change() {
    let repo = repository_with_actors(&["jane_example_com"]);
    create(repo.path(), "req_review", "requirement", &[]);
    create(repo.path(), "req_other", "requirement", &[]);
    let created = commit_all(repo.path(), "Create");
    let mut steps = Steps {
        log: GitLog::new(JANE),
        seen: Vec::new(),
    };
    let host = steps.log.start_host(repo.path());
    steps.record("host start");
    let jane = json!({"actorId": "jane_example_com", "allowed": true});

    assert_eq!(page_load(&host, "req_review"), jane);
    steps.record("first page load");
    for _ in 0..4 {
        assert_eq!(page_load(&host, "req_review"), jane);
    }
    steps.record("four later page loads");
    accept(&host, &pending_proposal(&host, "req_review"));
    steps.record("decision");
    comment(&host);
    steps.record("comment");
    edit(&host, "The system stores each result.");
    steps.record("first edit");
    edit(&host, "The system keeps each result.");
    steps.record("second edit");

    assert_eq!(
        history(&host, "req_review"),
        [committed(&created), working()]
    );
    steps.record("first history read");
    for _ in 0..2 {
        assert_eq!(
            history(&host, "req_review"),
            [committed(&created), working()]
        );
    }
    steps.record("two history reads at the same HEAD");

    let moved = commit_all(repo.path(), "Edit");
    assert_eq!(
        history(&host, "req_review"),
        [committed(&created), committed(&moved)]
    );
    steps.record("history read after HEAD moves");

    edit(&host, "The system deletes each result.");
    steps.record("working tree edit");
    let edited = [committed(&created), committed(&moved), working()];
    assert_eq!(history(&host, "req_review"), edited);
    steps.record("history read after the working tree edit");

    // The other Requirement is in the same record file, so the versions of the file are kept.
    assert_eq!(history(&host, "req_other"), [committed(&created)]);
    steps.record("history read of a second Requirement");
    assert_eq!(history(&host, "req_review"), edited);
    steps.record("history read of the first Requirement again");

    // The reviewer stays the same until the host restarts.
    steps.log.set_config("[user]\n\temail = john@example.com\n");
    assert_eq!(reviewer(&host), jane);
    steps.record("configuration read after an identity change");
    drop(host);
    let host = steps.log.start_host(repo.path());
    steps.record("host restart");
    assert_eq!(
        reviewer(&host),
        json!({"actorId": "john_example_com", "allowed": false})
    );
    steps.record("configuration read after the restart");
    assert_eq!(history(&host, "req_review"), edited);
    steps.record("history read after the restart");

    assert_eq!(steps.seen, expected_starts());
}

/// The Git subcommands that each step of
/// `review_host_starts_git_once_per_value_that_can_change` starts, in order.
fn expected_starts() -> Vec<(&'static str, Vec<String>)> {
    let first_read = ["--version", "rev-parse", "log", "cat-file", "cat-file"];
    let reviewer_read = ["config", "config"];
    expected(&[
        ("host start", &[]),
        ("first page load", &reviewer_read),
        ("four later page loads", &[]),
        ("decision", &[]),
        ("comment", &[]),
        ("first edit", &["rev-parse"]),
        ("second edit", &["rev-parse"]),
        ("first history read", &first_read),
        (
            "two history reads at the same HEAD",
            &["rev-parse", "rev-parse"],
        ),
        (
            "history read after HEAD moves",
            &["rev-parse", "log", "cat-file", "cat-file"],
        ),
        ("working tree edit", &["rev-parse"]),
        ("history read after the working tree edit", &["rev-parse"]),
        (
            "history read of a second Requirement",
            &["rev-parse", "cat-file"],
        ),
        (
            "history read of the first Requirement again",
            &["rev-parse", "cat-file"],
        ),
        ("configuration read after an identity change", &[]),
        ("host restart", &[]),
        ("configuration read after the restart", &reviewer_read),
        ("history read after the restart", &first_read),
    ])
}

#[test]
fn review_host_reads_history_again_after_a_shallow_clone_gets_deeper() {
    let origin = repository_with_actors(&["jane_example_com"]);
    create(origin.path(), "req_review", "requirement", &[]);
    let mut commits = vec![commit_all(origin.path(), "Create")];
    {
        let host = GitLog::new(JANE).start_host(origin.path());
        for statement in [
            "The system stores each result.",
            "The system keeps each result.",
        ] {
            edit(&host, statement);
            commits.push(commit_all(origin.path(), statement));
        }
    }
    let clone = tempfile::tempdir().unwrap();
    git(
        clone.path(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &format!("file://{}", origin.path().display()),
            ".",
        ],
    );
    let mut log = GitLog::new(JANE);
    let host = log.start_host(clone.path());
    assert_eq!(
        get(&host, "/requirements/req_review")["data"]["id"],
        "req_review"
    );
    log.step();

    assert_eq!(history(&host, "req_review"), [committed(&commits[2])]);
    let shallow = log.step();
    git(clone.path(), &["fetch", "-q", "--deepen", "2"]);
    assert_eq!(git(clone.path(), &["rev-parse", "HEAD"]), commits[2]);
    assert_eq!(
        history(&host, "req_review"),
        commits
            .iter()
            .map(|commit| committed(commit))
            .collect::<Vec<_>>()
    );
    let deeper = log.step();

    assert_eq!(
        [shallow, deeper],
        [
            ["--version", "rev-parse", "log", "cat-file", "cat-file"]
                .map(str::to_owned)
                .to_vec(),
            ["rev-parse", "log", "cat-file", "cat-file"]
                .map(str::to_owned)
                .to_vec(),
        ]
    );
}
