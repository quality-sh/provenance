//! The state faults of the tests of the refusal of a state file that does
//! not parse and of the two procedures that correct the state.
use serde_json::{json, Value};
use std::path::Path;

use super::review_paths::provenance;
use super::stale_repo::{commit, init_repo};

/// The hint of a record file that does not parse. The other tests compare
/// with the public constant; this file pins the text that a person reads.
pub const HINT: &str = "To correct this fault, use one of these two procedures. \
    Procedure 1: get .provenance/state from a commit where Provenance can read it. \
    This can remove records and changes that are not in that commit. \
    Procedure 2: remove .provenance/state and then do the command provenance init. \
    This removes all records, the scopes, the disposition actors and the technical names of \
    the dictionary.";

pub const REQUIREMENTS: &str = "scopes/default/requirements/req.jsonl";

/// A write that reads the Requirement file.
pub const WRITE: [&str; 6] = [
    "req_new",
    "create",
    "--type",
    "requirement",
    "--statement",
    "The system keeps a record.",
];

/// The exit code, the standard output and the standard error of a command.
pub fn run(repo: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let output = provenance().current_dir(repo).args(args).output().unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// The standard output of a command that succeeds.
pub fn passes(repo: &Path, args: &[&str]) -> String {
    let (code, stdout, stderr) = run(repo, args);
    assert_eq!(code, Some(0), "{args:?}: {stderr}");
    stdout
}

/// Creates the Requirement that `args` names, with the flags that follow it.
pub fn create(repo: &Path, args: &[&str]) {
    let statement = ["--statement", "The system keeps review feedback available."];
    let flags = ["create", "--type", "requirement"];
    passes(repo, &[&args[..1], &flags, &statement, &args[1..]].concat());
}

/// A Git repository with a commit of `req_parent` and `req_child`, where
/// `req_child` refines `req_parent`.
pub fn committed() -> tempfile::TempDir {
    let directory = init_repo();
    let repo = directory.path();
    create(repo, &["req_parent"]);
    create(repo, &["req_child", "--refines", "req_parent"]);
    commit(repo, "records");
    directory
}

/// Changes the `req_child` line, so that its `refines` is a string, as in a
/// line of an older form.
pub fn damage(repo: &Path) {
    let path = repo.join(".provenance/state").join(REQUIREMENTS);
    let text = std::fs::read_to_string(&path).unwrap();
    let changed = text.replace(r#""refines":["req_parent"]"#, r#""refines":"req_parent""#);
    assert_ne!(changed, text, "{text}");
    let first = r#"{"scope_id":"default","id":"req_child""#;
    assert!(changed.starts_with(first), "{changed}");
    std::fs::write(&path, changed).unwrap();
}

/// Removes the last character of the `req_child` line, as a person who
/// stopped an edit of the line.
pub fn cut(repo: &Path) {
    let path = repo.join(".provenance/state").join(REQUIREMENTS);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].contains(r#""id":"req_child""#), "{text}");
    lines[0] = &lines[0][..lines[0].len() - 1];
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
}

/// Changes the manifest, so that its disposition actors are one string and
/// not a list, as in a manifest of an older form. The string holds each actor
/// of the list. It gives the line and the parse message of the refusal.
pub fn older_manifest(repo: &Path) -> (u64, String) {
    let path = repo.join(".provenance/state/manifest.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let actors = manifest["disposition_actor_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|actor| actor.as_str().unwrap())
        .collect::<Vec<_>>()
        .join(" ");
    manifest["disposition_actor_ids"] = json!(actors);
    let text = serde_json::to_string_pretty(&manifest).unwrap();
    assert!(
        text.starts_with(&format!("{{\n  \"disposition_actor_ids\": \"{actors}\",")),
        "{text}"
    );
    std::fs::write(&path, text + "\n").unwrap();
    (2, format!("invalid type: string \"{actors}\", expected a sequence"))
}

/// Removes the last character of the manifest, as a person who stopped an
/// edit of the file. It gives the line and the parse message of the refusal.
pub fn cut_manifest(repo: &Path) -> (u64, String) {
    let path = repo.join(".provenance/state/manifest.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.trim_end();
    assert!(text.ends_with('}'), "{text}");
    let cut = &text[..text.len() - 1];
    std::fs::write(&path, cut).unwrap();
    let line = u64::try_from(cut.matches('\n').count() + 1).unwrap();
    (line, "EOF while parsing an object".to_owned())
}
