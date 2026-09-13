#![cfg(feature = "dogfood")]

//! Tests for `provenance dogfood simulate`: the simulation passes on a clean
//! sandbox, `--keep` is respected, and nothing is written outside the
//! sandbox.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Each test runs one full simulated onboarding, whose init imports a PDF
/// against a client with a 10s HTTP timeout. Serialize them, so a loaded
/// runner cannot starve a child into that timeout.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn simulate_command() -> Command {
    Command::cargo_bin("provenance").unwrap()
}

fn sandbox_parent(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let parent = temp.path().join("parent");
    std::fs::create_dir(&parent).unwrap();
    parent
}

#[test]
fn the_simulation_passes_on_a_clean_sandbox_and_removes_it() {
    let _guard = serial();
    let temp = tempfile::tempdir().unwrap();
    let parent = sandbox_parent(&temp);
    let spool = temp.path().join("spool");

    simulate_command()
        .env("PROVENANCE_DOGFOOD_DIR", &spool)
        .args(["dogfood", "simulate", "--dir", parent.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("simulated onboarding"))
        // The loopback fixture, not the network, served the dictionary.
        .stdout(predicate::str::contains(
            "Imported the official Issue 9 dictionary.",
        ))
        .stdout(predicate::str::contains("requirements create"))
        .stdout(predicate::str::contains("coverage scan"))
        .stdout(predicate::str::contains("verdict: pass"))
        .stdout(predicate::str::contains("sandbox removed"));

    let leftovers: Vec<_> = std::fs::read_dir(&parent)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        leftovers.is_empty(),
        "the simulation left entries outside the sandbox: {leftovers:?}"
    );
    // The simulator records no note and lets no child record one.
    assert!(!spool.exists());
}

#[test]
fn keep_preserves_the_sandbox_for_inspection() {
    let _guard = serial();
    let temp = tempfile::tempdir().unwrap();
    let parent = sandbox_parent(&temp);

    let stdout = simulate_command()
        .args([
            "dogfood",
            "simulate",
            "--keep",
            "--dir",
            parent.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("verdict: pass"))
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    let kept_line = stdout
        .lines()
        .find(|line| line.starts_with("sandbox kept at "))
        .expect("the report names the kept sandbox");
    let sandbox = Path::new(kept_line.trim_start_matches("sandbox kept at "));
    assert!(sandbox.is_dir(), "the kept sandbox is gone: {sandbox:?}");
    assert!(sandbox.join(".provenance").is_dir());
    assert!(sandbox
        .join("assets")
        .join("ASD-STE100_ISSUE9.pdf")
        .is_file());

    let leftovers: Vec<_> = std::fs::read_dir(&parent)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(leftovers.len(), 1, "the parent holds only the sandbox");
}

#[test]
fn the_simulation_writes_nothing_outside_the_sandbox() {
    let _guard = serial();
    let temp = tempfile::tempdir().unwrap();
    let parent = sandbox_parent(&temp);
    let spool = temp.path().join("spool");

    simulate_command()
        .env("PROVENANCE_DOGFOOD_DIR", &spool)
        .args([
            "dogfood",
            "simulate",
            "--keep",
            "--dir",
            parent.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("verdict: pass"));

    // With --keep the parent holds exactly one entry: the sandbox itself.
    let outside: Vec<String> = std::fs::read_dir(&parent)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let [only] = outside.as_slice() else {
        panic!("entries outside the sandbox: {outside:?}");
    };
    assert!(only.starts_with("provenance-simulate-"), "{outside:?}");

    // The note spool stays untouched: no notes.jsonl, nothing else.
    assert!(
        !spool.exists(),
        "the simulation touched the note spool: {:?}",
        std::fs::read_dir(&spool).map(std::iter::Iterator::count)
    );
}
