//! Characterization (pzm2): `provenance review` on a directory with no graph
//! refuses before it serves a request, in the foreground and with `--detach`.

mod support {
    pub mod review_paths;
}
use support::review_paths::provenance;

fn refusal(extra: &[&str]) -> (Option<i32>, Vec<String>) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    let output = provenance()
        .args(["review", "--no-open", "--repo", &repo])
        .args(["--repository-id", "r", "--scope", "default"])
        .args(extra)
        .output()
        .unwrap();
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8(output.stderr)
        .unwrap()
        .replace(&repo, "<repo>");
    (
        output.status.code(),
        stderr.lines().map(str::to_owned).collect(),
    )
}

#[test]
fn review_refuses_a_directory_with_no_graph() {
    assert_eq!(
        refusal(&[]),
        (
            Some(1),
            vec![
                "Error: cannot configure review repository access".to_owned(),
                String::new(),
                "Caused by:".to_owned(),
                "    Provenance graph is not initialized; run `provenance init`".to_owned(),
            ]
        )
    );
}

#[test]
fn detached_review_refuses_a_directory_with_no_graph() {
    assert_eq!(
        refusal(&["--detach"]),
        (
            Some(1),
            vec!["Error: Provenance graph is not initialized; run `provenance init`".to_owned()]
        )
    );
}
