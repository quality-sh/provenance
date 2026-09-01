#![cfg(not(feature = "dogfood"))]

use assert_cmd::Command;

#[test]
fn dogfood_command_does_not_exist_without_the_feature() {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["dogfood", "list"])
        .assert()
        .failure();
}

#[test]
fn dogfood_is_not_mentioned_in_help_without_the_feature() {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(output).unwrap();
    assert!(!help.contains("dogfood"));
}
