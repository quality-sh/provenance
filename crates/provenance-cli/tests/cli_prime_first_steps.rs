//! The first steps of CLI prime: their text, and each one runs to its end in a repository.
use crate::support::search_repository::{cli, repository, requirement};
use assert_cmd::Command;
use provenance_macros::verifies;

/// The words before the first-step commands of prime.
const BEFORE_COMMANDS: &str = "For a focused start, run:\n\n";

/// The first steps of prime, with the text before and after them.
const FIRST_STEPS: &str = "\n\nFor a focused start, run:\n\n\
- `provenance search --text <text> --scope <scope> --format json`\n\
- `provenance <id> get --view children --depth <N> --scope <scope> --format json`\n\n";

/// The record of the fixture repository.
const RECORD: &str = "req_first_step";

/// The arguments of each first-step command of `text`, after `provenance`, with an example
/// value for each placeholder: a count for `<N>`, the fixture record for `<id>`, the
/// default scope for `<scope>`, and a word of the fixture record for another placeholder.
fn first_step_commands(text: &str) -> Vec<Vec<String>> {
    let (_, steps) = text
        .split_once(BEFORE_COMMANDS)
        .expect("prime gives first steps");
    let steps = steps.split("\n\n").next().unwrap_or_default();
    steps
        .lines()
        .map(|line| {
            let command = line
                .strip_prefix("- `provenance ")
                .and_then(|line| line.strip_suffix('`'))
                .unwrap_or_else(|| panic!("{line} is not a provenance command"));
            command
                .split_whitespace()
                .map(|word| match word {
                    "<N>" => "1".to_owned(),
                    "<id>" => RECORD.to_owned(),
                    "<scope>" => "default".to_owned(),
                    word if word.starts_with('<') => "first".to_owned(),
                    word => word.to_owned(),
                })
                .collect()
        })
        .collect()
}

/// The first steps of prime, as the user sees them. This test is bound to no Rule: no Rule
/// states the text of the first steps.
#[test]
fn prime_gives_the_first_steps() {
    let repo = tempfile::tempdir().unwrap();
    let text = cli(&["prime", "--repo", repo.path().to_str().unwrap()]);
    assert!(text.contains(FIRST_STEPS), "{text}");
}

/// Each first-step command of prime, with an example value for each placeholder, runs to
/// its end in a repository that holds one record: it exits with zero and gives that record.
#[test]
#[verifies("rule_guidance_first_steps_are_accepted_calls", examples)]
fn each_first_step_command_of_prime_runs_and_gives_the_record() {
    let directory = tempfile::tempdir().unwrap();
    let repo = repository(
        directory.path(),
        Vec::new(),
        vec![requirement(RECORD, "The first step finds this record.")],
    );
    let text = cli(&["prime", "--repo", &repo]);
    let commands = first_step_commands(&text);
    assert!(commands.len() > 1, "{text}");
    for arguments in commands {
        let output = Command::cargo_bin("provenance")
            .unwrap()
            .current_dir(&repo)
            .args(&arguments)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains(&format!("\"{RECORD}\"")),
            "`provenance {}` exits with {:?}: {}{stdout}\n{text}",
            arguments.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
