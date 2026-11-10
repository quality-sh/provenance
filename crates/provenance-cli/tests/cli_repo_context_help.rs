use std::fmt::Write as _;

mod support {
    pub mod help_text;
}
use support::help_text::help;

#[test]
fn centralized_repository_context_keeps_command_help() {
    let commands: &[(&str, &[&str])] = &[
        ("export", &["export", "--help"]),
        ("import", &["import", "--help"]),
        (
            "swarm-backtrace-land",
            &["swarm-backtrace", "land", "--help"],
        ),
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
        // A Windows checkout can give the fixture CRLF line endings.
        include_str!("fixtures/repo_context_help.txt").replace("\r\n", "\n"),
        "command help changed from origin/main"
    );
}
