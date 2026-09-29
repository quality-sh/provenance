use std::fmt::Write as _;
use std::process::Command;

fn help(arguments: &[&str]) -> String {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(arguments)
        .output()
        .expect("run provenance help");
    assert!(
        output.status.success(),
        "help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("help output is UTF-8")
}

#[test]
fn centralized_repository_context_keeps_command_help() {
    let commands: &[(&str, &[&str])] = &[
        ("graph", &["graph", "requirement", "--help"]),
        ("traceability", &["traceability", "rule", "--help"]),
        ("gaps", &["gaps", "--help"]),
        ("health", &["health", "--help"]),
        ("orphans", &["orphans", "--help"]),
        ("export", &["export", "--help"]),
        ("import", &["import", "--help"]),
        (
            "graph-reference-issue",
            &["graph-reference", "issue", "--help"],
        ),
        (
            "swarm-backtrace-land",
            &["swarm-backtrace", "land", "--help"],
        ),
        ("wiki-build", &["wiki", "build", "--help"]),
        ("wiki-serve", &["wiki", "serve", "--help"]),
    ];
    let actual = commands
        .iter()
        .fold(String::new(), |mut output, (name, arguments)| {
            write!(output, "=== {name} ===\n{}", help(arguments))
                .expect("writing to a String cannot fail");
            output
        })
        .replace(' ', "·");

    assert_eq!(
        actual,
        include_str!("fixtures/repo_context_help.txt"),
        "command help changed from origin/main"
    );
}
