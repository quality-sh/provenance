//! A command that starts with an API collection name is a usage error that
//! shows the help command. Plumbing stays reachable through `provenance api`.

use assert_cmd::Command;
use provenance_macros::verifies;
use std::collections::BTreeSet;

/// Collection names that are also porcelain commands.
const PORCELAIN_ROOTS: [&str; 3] = ["api", "search", "discussions"];

fn collection_words() -> BTreeSet<&'static str> {
    provenance_contract::definitions()
        .iter()
        .filter_map(|definition| definition.path.split('/').nth(1))
        .filter(|word| !PORCELAIN_ROOTS.contains(word))
        .collect()
}

#[test]
#[verifies("rule_cli_collection_word_shows_help", exhaustion)]
#[verifies("rule_porcelain_plumbing_only_through_api", exhaustion)]
fn every_collection_word_is_a_usage_error_that_shows_the_help_command() {
    let directory = tempfile::tempdir().unwrap();
    let words = collection_words();
    assert!(words.contains("rules"), "{words:?}");
    for word in words {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
            .current_dir(directory.path())
            .args([word, "list", "--format", "json"])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{word}: {stderr}");
        assert!(output.stdout.is_empty(), "{word}");
        assert!(
            stderr.contains(&format!("'{word}' is an API collection")),
            "{word}: {stderr}"
        );
        assert!(stderr.contains("provenance --help"), "{word}: {stderr}");
    }

    let repo = directory.path().to_str().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["init", "--path", repo, "--scope", "default"])
        .assert()
        .success();
    let listed = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["api", "rules", "--repo", repo, "--format", "json"])
        .output()
        .unwrap();
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
}
