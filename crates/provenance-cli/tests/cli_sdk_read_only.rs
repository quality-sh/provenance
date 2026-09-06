#![cfg(target_os = "linux")]

#[path = "query_support/fixtures.rs"]
mod fixtures;

use provenance_macros::verifies;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn mount_available() -> bool {
    let output = Command::new("unshare")
        .args(["--user", "--map-root-user", "--mount", "true"])
        .output();
    match output {
        Ok(output) if output.status.success() => true,
        other => {
            eprintln!("SKIPPED read-only mount: user mount namespace unavailable: {other:?}");
            false
        }
    }
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.insert(path.clone(), std::fs::read(path).unwrap());
        }
    }
    found
}

fn mounted_answers(root: &Path, before: &Value, case: &str) {
    let cache = root.join(".provenance/cache");
    let database = cache.join("provenance.db");
    let bytes = std::fs::read(&database).unwrap();
    let canonical = files(&root.join(".provenance/state"));
    let marker = cache.join("import-publication.json");
    let pending = std::fs::read(&marker).ok();
    for policy in ["annotate_only", "catch_up", "refuse_stale"] {
        let output = assert_cmd::Command::new("unshare")
            .args(["--user", "--map-root-user", "--mount", "sh", "-ec"])
            .arg(
                "mount --bind \"$1\" \"$1\"\n\
                 mount -o remount,bind,ro \"$1\"\n\
                 exec \"$2\" sdk get --repo \"$1\" --freshness \"$3\"",
            )
            .arg("read-only-query")
            .arg(root)
            .arg(assert_cmd::cargo::cargo_bin!("provenance"))
            .arg(policy)
            .write_stdin(json!({"node_type": "requirement", "id": "req_missing"}).to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{case} {policy}: exit={:?}; {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let mut answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            answer["stamp"]["policy"],
            if policy == "catch_up" {
                "catch_up_failed"
            } else {
                policy
            }
        );
        if policy == "catch_up" {
            assert!(answer["freshness_error"].as_str().is_some());
            answer.as_object_mut().unwrap().remove("freshness_error");
        }
        answer["stamp"]["policy"] = before["stamp"]["policy"].clone();
        assert_eq!(answer, *before, "{case} {policy}");
        assert_eq!(std::fs::read(&database).unwrap(), bytes);
        assert_eq!(files(&root.join(".provenance/state")), canonical);
        assert_eq!(std::fs::read(&marker).ok(), pending);
        for suffix in ["wal", "shm"] {
            assert!(!cache.join(format!("provenance.db-{suffix}")).exists());
        }
    }
}

#[test]
#[verifies("rule_read_only_checkout_answers_as_an_immutable_image", examples)]
fn read_only_checkout_answers_without_a_publication_lock() {
    if !mount_available() {
        return;
    }
    for directory_missing in [false, true] {
        let repo = fixtures::init_repo();
        let before = fixtures::sdk(
            repo.path().to_str().unwrap(),
            "get",
            &json!({"node_type": "requirement", "id": "req_missing"}),
        );
        let locks = repo.path().join(".provenance/cache/locks");
        if directory_missing {
            std::fs::remove_dir_all(&locks).unwrap();
        } else {
            std::fs::remove_file(locks.join("repository.publication.lock")).unwrap();
        }
        mounted_answers(repo.path(), &before, "missing lock");
        assert!(!locks.join("repository.publication.lock").exists());
        assert_eq!(locks.exists(), !directory_missing);
    }
}

#[test]
#[verifies("rule_read_only_checkout_answers_as_an_immutable_image", examples)]
fn read_only_checkout_answers_with_a_valid_recovery_marker() {
    if !mount_available() {
        return;
    }
    for (phase, missing_lock) in ["prepared", "published"]
        .into_iter()
        .flat_map(|phase| [false, true].map(|missing| (phase, missing)))
    {
        let repo = fixtures::init_repo();
        let path = repo.path().to_str().unwrap();
        let request = json!({"node_type": "requirement", "id": "req_missing"});
        let before = fixtures::sdk(path, "get", &request);
        let state = repo.path().join(".provenance/state");
        let canonical = files(&state);
        let cache = repo.path().join(".provenance/cache");
        let transaction = cache.join("import-transactions").join(phase);
        if phase == "prepared" {
            let staged = transaction.join("staged-repo/.provenance/state");
            for (source, bytes) in &canonical {
                let target = staged.join(source.strip_prefix(&state).unwrap());
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(target, bytes).unwrap();
            }
        }
        let marker = cache.join("import-publication.json");
        std::fs::write(
            &marker,
            json!({"schema_version": 2, "transaction_dir": transaction, "phase": phase})
                .to_string(),
        )
        .unwrap();
        if missing_lock {
            std::fs::remove_dir_all(cache.join("locks")).unwrap();
        }
        mounted_answers(repo.path(), &before, phase);
        assert_eq!(cache.join("locks").exists(), !missing_lock);
        let recovered = fixtures::provenance()
            .args(["sdk", "get", "--repo", path, "--freshness", "refuse_stale"])
            .write_stdin(request.to_string())
            .output()
            .unwrap();
        assert!(recovered.status.success(), "{recovered:?}");
        let mut answer: Value = serde_json::from_slice(&recovered.stdout).unwrap();
        answer["stamp"]["policy"] = before["stamp"]["policy"].clone();
        assert_eq!(answer, before);
        assert_eq!(files(&state), canonical);
        assert!(!marker.exists());
        assert!(!transaction.exists());
    }
}
