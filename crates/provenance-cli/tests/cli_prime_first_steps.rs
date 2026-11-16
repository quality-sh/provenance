//! The first steps of CLI prime are commands that the CLI parser accepts.
use assert_cmd::Command;
use provenance_macros::verifies;

/// The words before the first-step commands of prime.
const FIRST_STEPS: &str = "For a focused start, run:\n\n";

/// The arguments of each first-step command of `text`, after `provenance`, with an example
/// value for each placeholder: a count for `<N>`, a record ID for `<id>`, and the default
/// scope word for another placeholder.
fn first_step_commands(text: &str) -> Vec<Vec<String>> {
    let (_, steps) = text
        .split_once(FIRST_STEPS)
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
                    "<id>" => "req_first_step".to_owned(),
                    word if word.starts_with('<') => "default".to_owned(),
                    word => word.to_owned(),
                })
                .collect()
        })
        .collect()
}

/// The CLI parser accepts each first-step command of prime, with an example value for each
/// placeholder. Each command runs with `--help` at its end, so the parser checks each
/// argument and value before it and the command reads no repository.
#[test]
#[verifies("rule_guidance_first_steps_are_accepted_calls", examples)]
fn the_cli_parser_accepts_each_first_step_command_of_prime() {
    let repo = tempfile::tempdir().unwrap();
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(repo.path())
        .arg("prime")
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    let commands = first_step_commands(&text);
    assert!(commands.len() > 1, "{text}");
    for arguments in commands {
        let output = Command::cargo_bin("provenance")
            .unwrap()
            .current_dir(repo.path())
            .args(&arguments)
            .arg("--help")
            .output()
            .unwrap();
        assert!(
            output.status.success() && !output.stdout.is_empty(),
            "the CLI parser refuses `provenance {}`: {}\n{text}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
