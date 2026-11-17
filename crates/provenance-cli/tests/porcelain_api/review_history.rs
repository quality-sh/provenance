use super::*;

/// Runs one Git command on `repo` and gives its output, whatever its status.
pub fn git_output(repo: &str, args: &[&str]) -> std::process::Output {
    provenance_store::git_process::git()
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
        .unwrap()
}

pub fn git(repo: &str, args: &[&str]) -> String {
    let result = git_output(repo, args);
    assert!(result.status.success(), "git {args:?} failed: {result:?}");
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

pub fn commit(repo: &str) -> String {
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

/// Flow: create and commit, put a directory at the path of the record file
/// and commit, then put the file back with a new description and commit. The
/// version with the directory has no record, so the history lists only the
/// two versions with a file.
#[test]
#[verifies("rule_record_history_text_only_from_file", examples)]
fn a_version_with_a_directory_at_the_path_of_the_record_file_has_no_record() {
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
    let file =
        std::path::Path::new(&repo).join(".provenance/state/scopes/default/requirements/req.jsonl");
    let bytes = std::fs::read(&file).unwrap();
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    std::fs::write(file.join("inner"), "inner\n").unwrap();
    commit(&repo);
    std::fs::remove_dir_all(&file).unwrap();
    std::fs::write(&file, bytes).unwrap();
    describe(&repo, "After the directory.");
    let described = commit(&repo);

    let history = json(&["api", "requirements/req_history/history", "--repo", &repo]);
    let ids = history["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(ids, [created, described]);
}

/// Flow: create and commit, commit a line that is not one JSON value, then a
/// line that holds a top-level member two times, then the file as it was.
/// The history lists the one version and gives one warning for each refused
/// line, in the order of the commits.
#[test]
fn history_warns_about_each_refused_committed_line() {
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
    let path = ".provenance/state/scopes/default/requirements/req.jsonl";
    let file = std::path::Path::new(&repo).join(path);
    let text = std::fs::read_to_string(&file).unwrap();
    let line = text.lines().count() + 1;
    std::fs::write(&file, format!("{text}<<<<<<< HEAD\n")).unwrap();
    let marker = commit(&repo);
    let repeated_line = r#"{"id":"req_other","id":"req_again"}"#;
    std::fs::write(&file, format!("{text}{repeated_line}\n")).unwrap();
    let repeated = commit(&repo);
    std::fs::write(&file, &text).unwrap();
    commit(&repo);

    let history = json(&["api", "requirements/req_history/history", "--repo", &repo]);
    let ids = history["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(ids, [created], "{history}");
    assert_eq!(
        history["data"]["warnings"],
        json!([
            {"commit": marker, "path": path, "line": line,
                "message": "The line is not one JSON value."},
            {"commit": repeated, "path": path, "line": line,
                "message": "The line holds the top-level member \"id\" two times."},
        ]),
        "{history}"
    );
}
