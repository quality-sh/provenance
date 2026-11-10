//! Checks the report of the bounded process helper of the CLI tests.

#[path = "support/bounded_process.rs"]
mod bounded_process;

// Implementation aid: a stopped CLI command must fail fast with the lock it waits for and the lock holder.
// No Rule applies: the test checks test support, not product behaviour.
#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread")]
async fn a_command_past_its_limit_names_the_lock_and_its_holder() {
    use std::process::Command;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_str().unwrap().to_owned();
    let mut init = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
    init.args(["init", "--path", &root, "--scope", "default"])
        .args(["--path-prefix", "."]);
    let output = bounded_process::output(&mut init, bounded_process::LIMIT);
    assert!(output.status.success(), "{output:?}");
    let repository =
        provenance_store::repository::Repository::open(directory.path(), "default").unwrap();
    let guard = provenance_store::publication::publication_guard(&repository)
        .await
        .unwrap();

    let failure = tokio::task::spawn_blocking(move || {
        let mut create = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
        create
            .args([
                "source_held",
                "create",
                "--type",
                "source",
                "--name",
                "Held",
            ])
            .args(["--repo", &root]);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            bounded_process::output(&mut create, std::time::Duration::from_secs(20))
        }))
    })
    .await
    .unwrap()
    .expect_err("the write waits for the held publication lock");
    drop(guard);

    let report = failure.downcast_ref::<String>().expect("a panic message");
    let (process, locks) = report
        .split_once("locks on the open files:")
        .expect("a lock section");
    assert!(process.contains("did not end within 20s"), "{report}");
    assert!(process.contains("publication.lock"), "{report}");
    let holder = format!("process {}: ", std::process::id());
    let held = locks.lines().find(|line| line.contains(&holder));
    assert!(held.is_some_and(|line| line.contains("FLOCK")), "{report}");
    assert!(
        locks.lines().any(|line| line.contains("-> FLOCK")),
        "{report}"
    );
}
