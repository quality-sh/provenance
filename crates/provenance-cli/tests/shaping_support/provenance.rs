use assert_cmd::Command;

pub fn provenance(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .assert()
}

#[allow(dead_code)]
pub fn provenance_stdin(args: &[&str], input: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .write_stdin(input)
        .assert()
}
