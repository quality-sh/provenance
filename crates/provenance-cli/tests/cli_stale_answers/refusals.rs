use crate::support::{cli_stale, commit, fixture, git, remove_object, write, Goldens};
use provenance_macros::verifies;
use serde_json::json;

/// The exit code, the JSON error and the diagnostic text of a refused
/// request, with the commit ids replaced by names.
fn refusal(fixture: &crate::support::Fixture, query: &[(&str, &str)]) -> serde_json::Value {
    let (code, stdout, stderr) = cli_stale(fixture.path(), query);
    let stderr = stderr
        .replace(&fixture.base, "BASE")
        .replace(&fixture.head, "HEAD");
    fixture.normalized(&json!({"code": code, "stdout": stdout, "stderr": stderr}))
}

/// The Rule covers the cause of the refusal for a source file that is not
/// UTF-8. No Rule covers the other refusals, which have a known kind; the
/// test keeps their exit code and text the same.
#[test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
fn refused_requests_give_the_recorded_refusals() {
    let mut fixture = fixture();
    let mut goldens = Goldens::default();
    goldens.check(
        "refusal_unknown_base",
        &refusal(&fixture, &[("base", "no-such-revision")]),
    );
    let tree = git(fixture.path(), &["rev-parse", "HEAD:"]);
    let mut tree_refusal = refusal(&fixture, &[("base", tree.as_str())]);
    tree_refusal = serde_json::from_str(&tree_refusal.to_string().replace(&tree, "TREE")).unwrap();
    goldens.check("refusal_tree_base", &tree_refusal);
    let base = fixture.base.clone();
    goldens.check(
        "refusal_unknown_head",
        &refusal(
            &fixture,
            &[("base", base.as_str()), ("head", "no-such-revision")],
        ),
    );
    goldens.check(
        "refusal_zero_limit",
        &refusal(&fixture, &[("base", base.as_str()), ("limit", "0")]),
    );

    write(
        fixture.path(),
        "src/latin1.rs",
        [b'f', b'n', b' ', 0xe9, b'(', b')', b'\n'],
    );
    fixture.head = commit(fixture.path(), "a source file that is not UTF-8");
    let head = fixture.head.clone();
    goldens.check(
        "refusal_source_not_utf8",
        &refusal(
            &fixture,
            &[("base", base.as_str()), ("head", head.as_str())],
        ),
    );
    goldens.finish();
}

/// Two file diffs of the range fail. The refusal gives the failure of the
/// first file in diff order as its cause.
#[test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
fn the_first_failed_file_diff_in_diff_order_wins() {
    let mut fixture = fixture();
    fixture.base = fixture.head.clone();
    let base = fixture.base.clone();
    write(fixture.path(), "later.txt", "Later.\n");
    write(fixture.path(), "notes.md", "Notes, changed again.\n");
    fixture.head = commit(fixture.path(), "two files whose blobs are lost");
    let head = fixture.head.clone();
    // The two files are not source files, so the query reads their blobs
    // only in the file diffs.
    for path in ["later.txt", "notes.md"] {
        let blob = git(fixture.path(), &["rev-parse", &format!("{head}:{path}")]);
        remove_object(fixture.path(), &blob);
    }
    let mut goldens = Goldens::default();
    goldens.check(
        "refusal_file_diff",
        &refusal(
            &fixture,
            &[("base", base.as_str()), ("head", head.as_str())],
        ),
    );
    goldens.finish();
}
