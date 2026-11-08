use super::{failure_cause, in_repository};
use provenance_macros::verifies;
use std::path::Path;

fn cause(root: Option<&str>, native: &str) -> String {
    let error = anyhow::anyhow!("{native}");
    let error = match root {
        Some(root) => in_repository(Path::new(root), error),
        None => error,
    };
    failure_cause(&error)
}

/// A path below the repository root is relative to that root, with `/`
/// between its parts, for each spelling of the root.
#[test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
fn a_path_below_the_repository_is_relative_to_the_root() {
    for (root, native, expected) in [
        (
            "/tmp/a /b/repo",
            "load /tmp/a /b/repo/.provenance/state/scopes/x.jsonl line 1: bad",
            "load .provenance/state/scopes/x.jsonl line 1: bad",
        ),
        (
            "/home/u/r",
            "/home/u/r/.provenance/cache/import-transactions/t1/staged-repo/.provenance/state/x.jsonl line 1",
            ".provenance/cache/import-transactions/t1/staged-repo/.provenance/state/x.jsonl line 1",
        ),
        (
            "/home/u/r",
            "path=/home/u/r/.provenance/state/a.jsonl: bad",
            "path=.provenance/state/a.jsonl: bad",
        ),
        (
            r"\\?\C:\Users\Jo Smith\r",
            r"load: \\?\C:\Users\Jo Smith\r\.provenance\state\scopes\x.jsonl line 1",
            "load: .provenance/state/scopes/x.jsonl line 1",
        ),
        (
            r"\\?\C:\Users\Jo Smith\r",
            r"lock C:\Users\Jo Smith\r\.provenance\cache\close.lock: denied",
            "lock .provenance/cache/close.lock: denied",
        ),
        (
            r"C:\Users\Jo Smith\r",
            r"open C:/Users/Jo Smith/r/.provenance/cache/close.lock: denied",
            "open .provenance/cache/close.lock: denied",
        ),
        ("/home/u/r", "no repository at /home/u/r", "no repository at ."),
    ] {
        assert_eq!(cause(Some(root), native), expected, "{native}");
    }
}

/// An absolute path that is not below the repository root is only its file
/// name. With no known root, a path that holds a space keeps the part after
/// the space, but no part of it is absolute.
#[test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
fn a_path_outside_the_repository_is_only_its_file_name() {
    for (root, native, expected) in [
        (
            Some("/home/u/r"),
            "open /var/lock/close.lock: denied",
            "open close.lock: denied",
        ),
        (
            Some("/home/u/r"),
            "copy /home/u/r2/x.jsonl to '/tmp/y.jsonl'",
            "copy x.jsonl to 'y.jsonl'",
        ),
        (
            Some("/home/u/r"),
            r"lock C:\Temp\close.lock: denied",
            "lock close.lock: denied",
        ),
        (
            None,
            "load /tmp/a /b/repo/.provenance/state/x.jsonl line 1: bad",
            "load a x.jsonl line 1: bad",
        ),
        (
            None,
            r"load: \\?\C:\Users\Jo Smith\r\.provenance\state\x.jsonl line 1",
            r"load: Jo Smith\r\.provenance\state\x.jsonl line 1",
        ),
        (
            None,
            "see https://example.com/a and a / b",
            "see https://example.com/a and a / b",
        ),
    ] {
        assert_eq!(cause(root, native), expected, "{native}");
    }
}
