#![cfg(target_os = "linux")]

#[path = "query_support/fixtures.rs"]
mod fixtures;

use provenance_macros::verifies;
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

const READ_ONLY_GET: &str = r#"
    inspect() {
        echo "$1" >&2
        findmnt -T "$cache" -o TARGET,FSTYPE,VFS-OPTIONS >&2
        ls -la "$cache" >&2
    }
    skip() { echo "SKIPPED: $1" >&2; exit 77; }
    cache=$1
    inspect before-bind
    mount --bind "$cache" "$cache" || skip 'bind mount failed'
    inspect after-bind
    mount -o remount,bind,ro "$cache" || skip 'read-only remount failed'
    inspect after-remount
    options=$(findmnt -n -T "$cache" -o VFS-OPTIONS) || skip 'cannot read mount options'
    case ",$options," in
        *,ro,*) ;;
        *) skip "mount is not read-only: $options" ;;
    esac
    if ( : > "$cache/mount-probe" ); then
        rm -f "$cache/mount-probe"
        skip 'the directory still permits file creation'
    fi
    if ( : >> "$cache/provenance.db" ); then
        skip 'the database still permits a write open'
    fi
    "$2" sdk get --repo "$3" --freshness "$4"
    result=$?
    inspect after-read
    exit "$result"
"#;

#[test]
#[verifies("rule_read_only_checkout_answers_as_an_immutable_image", examples)]
fn a_read_only_mount_answers_without_the_publication_lock() {
    let available = Command::new("unshare")
        .args(["--user", "--map-root-user", "--mount", "true"])
        .output();
    if !available.is_ok_and(|output| output.status.success()) {
        skip("unshare cannot make a private mount namespace here");
        return;
    }
    let repo = fixtures::init_repo();
    let ids = fixtures::apply_shared_rule(&repo);
    let path = repo.path().to_str().unwrap();
    let input = json!({"node_type": "requirement", "id": ids.sharing});
    let before = fixtures::sdk(path, "get", &input);
    assert_eq!(before["found"], true);
    assert_eq!(before["stamp"]["policy"], "catch_up");
    assert!(before.get("freshness_error").is_none());
    let cache = repo.path().join(".provenance/cache");
    assert!(
        !cache.join("provenance.db-wal").exists(),
        "the read before mounting left the -wal file"
    );
    assert!(
        !cache.join("provenance.db-shm").exists(),
        "the read before mounting left the -shm file"
    );
    let binary = assert_cmd::cargo::cargo_bin!("provenance");
    for (policy, outcome) in [
        ("annotate_only", "annotate_only"),
        ("catch_up", "catch_up_failed"),
    ] {
        let mut child = Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "sh",
                "-c",
                READ_ONLY_GET,
                "read-only-mount",
            ])
            .arg(&cache)
            .arg(binary)
            .arg(path)
            .arg(policy)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let sent = child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes());
        let output = child.wait_with_output().unwrap();
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        if output.status.code() == Some(77) {
            skip(&diagnostics);
            return;
        }
        eprintln!("{policy}: {diagnostics}");
        assert!(output.status.success(), "{policy}: {diagnostics}");
        sent.unwrap();
        let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(answer["found"], true);
        assert_eq!(answer["node"], before["node"]);
        assert_eq!(answer["stamp"]["serial"], before["stamp"]["serial"]);
        assert_eq!(answer["stamp"]["digest"], before["stamp"]["digest"]);
        assert_eq!(answer["stamp"]["policy"], outcome);
        assert_eq!(
            answer.get("freshness_error").is_some(),
            policy == "catch_up"
        );
        if policy == "catch_up" {
            assert!(answer["freshness_error"]
                .as_str()
                .unwrap()
                .contains("Read-only file system"));
        }
        assert!(!cache.join("provenance.db-wal").exists());
        assert!(!cache.join("provenance.db-shm").exists());
    }
}

fn skip(reason: &str) {
    // Keep the skip visible when the test runner captures normal output.
    let mut stderr = std::io::stderr().lock();
    writeln!(
        stderr,
        "SKIPPED a_read_only_mount_answers_without_the_publication_lock: {reason}"
    )
    .unwrap();
}
