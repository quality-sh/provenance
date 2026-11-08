use crate::support::{cli_stale, commit, fixture, git, write, Goldens};
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

#[test]
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
