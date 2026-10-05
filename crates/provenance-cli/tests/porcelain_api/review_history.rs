use super::*;

pub(super) fn git(repo: &str, args: &[&str]) -> String {
    let result = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Reviewer",
            "-c",
            "user.email=reviewer@example.com",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(result.status.success(), "git {args:?} failed: {result:?}");
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

pub(super) fn commit(repo: &str) -> String {
    git(repo, &["add", ".provenance/state"]);
    git(repo, &["commit", "-q", "-m", "Save the graph"]);
    git(repo, &["rev-parse", "HEAD"])
}

fn describe(repo: &str, description: &str) {
    let read = json(&["api", "requirements/req_history", "--repo", repo]);
    let etag = read["data"]["edit"]["etag"].as_str().unwrap();
    let result = provenance()
        .args([
            "api",
            "requirements/req_history",
            "--repo",
            repo,
            "--method",
            "patch",
            "--input",
            "-",
            "--header",
            &format!("If-Match: {etag}"),
        ])
        .write_stdin(json!({"actor":"agent","description":description}).to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn evidence(repo: &str, side: &str) -> Value {
    json(&[
        "api",
        &format!("requirements/req_history/history/working/evidence/{side}"),
        "--repo",
        repo,
        "--query",
        "field=description",
    ])
}

/// Flow: create, edit twice with a commit between each write, then read the
/// history and the evidence of the last version through the binary.
#[test]
#[verifies("rule_record_history_reads_git", examples)]
fn history_and_evidence_read_commits_and_the_working_copy() {
    let (_directory, repo) = init();
    git(&repo, &["init", "-q"]);
    success(&[
        "req_history",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The history statement applies.",
    ]);
    let created = commit(&repo);
    describe(&repo, "First description.");
    let described = commit(&repo);
    describe(&repo, "Second description.");

    let history = json(&["api", "requirements/req_history/history", "--repo", &repo]);
    let ids = history["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(ids, [created, described, "working".to_owned()]);
    assert_eq!(
        evidence(&repo, "before")["data"]["json_text"],
        "\"First description.\""
    );
    assert_eq!(
        evidence(&repo, "after")["data"]["json_text"],
        "\"Second description.\""
    );
}
