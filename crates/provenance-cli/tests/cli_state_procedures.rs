//! Each of the two procedures that the refusal of a state file that does not
//! parse names, done as the refusal says, gives a state that Provenance can
//! read. The faults are a Requirement line of an older form, a Requirement
//! line that a person cut, and a manifest of each of these two forms.
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

use crate::support;
use support::stale_repo::git;
use support::state_faults::{
    committed, create, cut, cut_manifest, damage, older_manifest, passes, run, HINT, WRITE,
};

/// The faults of the state: a `req_child` line of an older form, a
/// `req_child` line that a person cut, and the two faults of a manifest.
const FAULTS: [fn(&Path); 4] = [
    damage,
    cut,
    |repo| {
        older_manifest(repo);
    },
    |repo| {
        cut_manifest(repo);
    },
];

/// The hint of the refusal of a write.
fn hint(repo: &Path) -> Value {
    let (code, _, stderr) = run(repo, &[&WRITE[..], &["--format", "json"]].concat());
    assert_eq!(code, Some(1), "{stderr}");
    serde_json::from_str::<Value>(&stderr).unwrap()["error"]["hint"].take()
}

/// The JSON get of `id`, which passes with no freshness warning.
fn read(repo: &Path, id: &str) -> Value {
    let read = passes(repo, &[id, "get", "--format", "json"]);
    let read: Value = serde_json::from_str(&read).unwrap();
    let warning = &read["record_metadata"]["freshness_error"];
    assert_eq!(warning, &Value::Null, "{read}");
    read
}

/// Writes a dictionary reference with a technical name to the state. The CLI
/// has no command that adds a technical name.
fn add_technical_name(repo: &Path) {
    let dictionary = crate::dictionary_support::imported_dictionary();
    crate::dictionary_support::write_reference_with_technical_names(
        repo,
        dictionary,
        &["Provenance"],
    );
}

/// The JSON file at `file` below the state directory.
fn state_json(repo: &Path, file: &str) -> Value {
    let path = repo.join(".provenance/state").join(file);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// The technical names of the dictionary reference of the state, when it has
/// a reference.
fn technical_names(repo: &Path) -> Option<Value> {
    let path = repo.join(".provenance/state/dictionary.json");
    let bytes = std::fs::read(path).ok()?;
    let reference: Value = serde_json::from_slice(&bytes).unwrap();
    reference.get("technical_names").cloned()
}

/// Procedure 2, done as the hint says: remove .provenance/state and then do
/// the command provenance init. After them, the state reads, and it holds none
/// of the records, scopes, disposition actors and technical names that it held.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn init_after_the_removal_of_the_state_gives_a_state_that_reads() {
    for fault in FAULTS {
        let directory = committed();
        let repo = directory.path();
        std::fs::create_dir(repo.join("other")).unwrap();
        support::review_paths::init_scope(repo, "other", "other");
        passes(repo, &["init", "--disposition-actor-id", "approver"]);
        fault(repo);
        let manifest = repo.join(".provenance/state/manifest.json");
        let faulty = std::fs::read_to_string(&manifest).unwrap();
        assert!(faulty.contains("approver"), "{faulty}");
        assert_eq!(hint(repo), json!(HINT));
        add_technical_name(repo);
        assert_eq!(technical_names(repo), Some(json!(["Provenance"])));

        std::fs::remove_dir_all(repo.join(".provenance/state")).unwrap();
        passes(repo, &["init"]);

        let manifest = state_json(repo, "manifest.json");
        let scopes = json!([{"id": "default", "path_prefix": "."}]);
        assert_eq!(manifest["scopes"], scopes, "{manifest}");
        let actors = manifest["disposition_actor_ids"].as_array().unwrap();
        assert!(!actors.contains(&json!("approver")), "{manifest}");
        assert_eq!(technical_names(repo), None);
        let (code, stdout, stderr) = run(repo, &["req_child", "get", "--format", "json"]);
        assert_eq!(code, Some(1), "{stdout}");
        let refused: Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(refused["error"]["kind"], "resource_not_found", "{stderr}");
        create(repo, &["req_new"]);
        let read = read(repo, "req_new");
        assert_eq!(read["record"]["id"], "req_new", "{read}");
    }
}

/// Procedure 1, done as the hint says: get .provenance/state from a commit
/// where Provenance can read it. The state then reads and holds the records of
/// that commit, and a record that is not in that commit is removed.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn the_state_of_a_commit_where_it_reads_gives_a_state_that_reads() {
    for fault in FAULTS {
        let directory = committed();
        let repo = directory.path();
        create(repo, &["req_extra"]);
        fault(repo);
        assert_eq!(hint(repo), json!(HINT));

        git(repo, &["checkout", "HEAD", "--", ".provenance/state"]);

        let read = read(repo, "req_child");
        let refines = &read["record"]["value"]["refines"];
        assert_eq!(refines, &json!(["req_parent"]), "{read}");
        let (code, _, stderr) = run(repo, &["req_extra", "get", "--format", "json"]);
        assert_eq!(code, Some(1), "{stderr}");
        let refused: Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(refused["error"]["kind"], "resource_not_found", "{stderr}");
    }
}
