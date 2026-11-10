//! TypeScript initialization for each supported package manager: each command
//! that init prints or writes in AGENTS.md uses the project package, and the
//! package manager gives each argument to Provenance.

use provenance_macros::verifies;
use pulldown_cmark::{Event, Parser};
use std::path::{Path, PathBuf};
use std::process::Output;

#[path = "cli_dictionary/support.rs"]
#[allow(dead_code)]
mod dictionary_support;
#[path = "cli_init_typescript/support.rs"]
#[allow(dead_code)]
mod support;

use support::{link_project_binary, on_path, shell, write_package_json, MANAGERS, PACKAGE_BIN};

/// Yarn finds a binary only through a real package installation, so this test
/// does not run its commands with the project binary.
const RUN_WITH_PROJECT_BINARY: [&str; 5] = ["npm", "pnpm", "bun", "deno", "nub"];

#[test]
#[verifies("rule_init_typescript_local_command", examples)]
fn typescript_init_prints_and_writes_package_manager_commands() {
    // Each package manager gets the two reviewer branches: a Git identity
    // gives the reviewer notice, and no identity gives the read-only warning.
    for (manager, executable, prefix) in MANAGERS {
        for git_identity in [true, false] {
            let temporary = tempfile::tempdir().unwrap();
            let repo = typescript_project(temporary.path(), git_identity);
            let output = init_typescript(temporary.path(), &repo, manager);
            let label = format!("{manager}, Git identity {git_identity}");
            assert!(
                output.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let printed = printed(&output);
            let reviewer = if git_identity {
                "Reviewer set to \"reviewer_example_com\". Change reviewers with `"
            } else {
                "Warning: No reviewer is configured. Review will be read-only. Add a reviewer with `"
            };
            assert!(printed.contains(reviewer), "{label}: {printed}");

            let mut commands = printed_commands(&label, prefix, &printed);
            let expected_printed = [
                format!("{prefix} prime"),
                format!("{prefix} dictionary import --pdf <path>"),
                format!(
                    "{prefix} init --path {} --disposition-actor-id <reviewer-id>",
                    repo.display()
                ),
            ];
            for expected in &expected_printed {
                assert!(commands.contains(expected), "{label}: {expected}");
            }
            assert_eq!(commands.len(), expected_printed.len(), "{label}: {printed}");
            commands.extend(section_commands(&label, &repo, prefix));

            if !git_identity {
                continue;
            }
            if cfg!(unix) && RUN_WITH_PROJECT_BINARY.contains(&manager) && on_path(executable) {
                run_commands(&repo, manager, prefix, &commands);
                eprintln!("{manager}: ran the commands");
            } else {
                eprintln!("{manager}: checked the text only");
            }
        }
    }
}

#[test]
#[verifies("rule_init_typescript_local_command", examples)]
fn typescript_reinit_hands_off_with_the_package_manager_command() {
    let (manager, _, prefix) = MANAGERS[0];
    let temporary = tempfile::tempdir().unwrap();
    let repo = typescript_project(temporary.path(), true);
    assert!(init_typescript(temporary.path(), &repo, manager)
        .status
        .success());

    let output = init_typescript(temporary.path(), &repo, manager);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(
        stdout.starts_with(&format!(
            "Provenance is already set up for scope \"default\" in {}. No change.\n",
            repo.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.ends_with(&format!(
            "\nHave your agent run {prefix} prime to get acclimated.\n"
        )),
        "{stdout}"
    );
    printed_commands(manager, prefix, &printed(&output));
}

#[test]
#[verifies("rule_init_typescript_local_command", examples)]
fn typescript_skill_conflict_names_the_package_manager_command() {
    let (manager, _, prefix) = MANAGERS[0];
    let temporary = tempfile::tempdir().unwrap();
    let repo = typescript_project(temporary.path(), true);
    assert!(init_typescript(temporary.path(), &repo, manager)
        .status
        .success());
    let skill = repo.join(".agents/skills/provenance-shaping/SKILL.md");
    let mut edited = std::fs::read_to_string(&skill).unwrap();
    edited.push_str("\nUser edit.\n");
    std::fs::write(&skill, &edited).unwrap();

    let output = init_typescript(temporary.path(), &repo, manager);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(&format!(
            "SKILL.md exists and differs; run `{prefix} skills install --force` with this working directory: {}",
            repo.display()
        )),
        "{stderr}"
    );
    assert_eq!(std::fs::read_to_string(skill).unwrap(), edited);
}

/// Checks the managed AGENTS.md section and returns its Provenance commands.
fn section_commands(label: &str, repo: &Path, prefix: &str) -> Vec<String> {
    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    let section = agents.split_once("## Provenance\n").unwrap().1;
    let note = format!(
        "- The installed skills and the output of each command show commands as\n  `provenance …`. In this project, run them as `{prefix} …`\n  from the directory that contains `node_modules`.\n"
    );
    assert!(
        section.contains(&format!(
            "development dependency\n  with the package manager.\n{note}"
        )),
        "{label}: {section}"
    );
    assert_eq!(section.matches("The installed skills").count(), 1);
    for collection_form in [
        "provenance rules ",
        "provenance requirements ",
        "provenance questions ",
        "provenance topics ",
    ] {
        assert!(
            !section.contains(collection_form),
            "{label}: {collection_form}"
        );
    }
    let commands = provenance_commands(&section.replacen(&note, "", 1));
    for command in &commands {
        assert_prefixed(label, prefix, command);
    }
    for expected in [
        "--version",
        "prime --quiet",
        "check --quiet",
        "check --statement \"<statement>\" --format json",
    ] {
        assert!(
            commands.contains(&format!("{prefix} {expected}")),
            "{label}: {expected}"
        );
    }
    let create = format!("{prefix} rule_<slug> create --type rule ");
    assert!(
        commands.iter().any(|command| command.starts_with(&create)),
        "{label}: {create}"
    );
    commands
}

/// Returns the commands in the printed text and in the handoff line, and
/// requires that each one starts with the package manager command.
fn printed_commands(label: &str, prefix: &str, printed: &str) -> Vec<String> {
    let mut commands = provenance_commands(printed);
    commands.extend(handoff_command(printed));
    for command in &commands {
        assert_prefixed(label, prefix, command);
    }
    commands
}

fn assert_prefixed(label: &str, prefix: &str, command: &str) {
    assert!(
        command.starts_with(&format!("{prefix} ")),
        "{label}: {command}"
    );
    let address = command[prefix.len()..].split_whitespace().next();
    assert!(
        !matches!(
            address,
            Some("rules" | "requirements" | "questions" | "topics")
        ),
        "{label}: {command}"
    );
}

fn printed(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Makes a Git repository with a package.json and the CLI under test at the
/// package binary path. The Git configuration of the run is in the temporary
/// directory, with or without a user identity.
fn typescript_project(temporary: &Path, git_identity: bool) -> PathBuf {
    let repo = temporary.join("repo");
    std::fs::create_dir(&repo).unwrap();
    write_package_json(&repo);
    link_project_binary(&repo);
    let identity = if git_identity {
        "[user]\n\temail = reviewer@example.com\n"
    } else {
        ""
    };
    std::fs::write(temporary.join("gitconfig"), identity).unwrap();
    let git = provenance_store::git_process::git()
        .args(["init", "--quiet"])
        .current_dir(&repo)
        .env("GIT_CONFIG_GLOBAL", temporary.join("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(git.success());
    repo
}

/// The dictionary download fails, so init prints the dictionary warning.
fn init_typescript(temporary: &Path, repo: &Path, manager: &str) -> Output {
    dictionary_support::init_command(
        repo,
        &[
            "--invocation-channel",
            "typescript",
            "--package-manager",
            manager,
        ],
    )
    .env("GIT_CONFIG_GLOBAL", temporary.join("gitconfig"))
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("PROVENANCE_STE100_ASSET_DIR", temporary.join("assets"))
    .env("PROVENANCE_STE100_INDEX_DIR", temporary.join("indexes"))
    .env(
        "PROVENANCE_TEST_STE100_ASSET_URL",
        env!("PROVENANCE_TEST_STE100_ASSET_URL"),
    )
    .output()
    .unwrap()
}

/// Runs the version and help commands, and each written command that has no
/// placeholder and does not start a review host, with the package manager.
fn run_commands(repo: &Path, manager: &str, prefix: &str, commands: &[String]) {
    let version = shell(repo, &format!("{prefix} --version"))
        .output()
        .unwrap();
    let version = String::from_utf8_lossy(&version.stdout);
    let release = format!("provenance {}", env!("CARGO_PKG_VERSION"));
    assert_eq!(
        version
            .lines()
            .next()
            .and_then(|line| line.split(" (").next()),
        Some(release.as_str()),
        "{manager}: {version}"
    );
    let help = shell(repo, &format!("{prefix} --help")).output().unwrap();
    assert!(
        String::from_utf8_lossy(&help.stdout).contains("Usage: provenance"),
        "{manager}: {}",
        String::from_utf8_lossy(&help.stdout)
    );
    for command in commands {
        if command.contains('<') || command.contains(" review ") {
            continue;
        }
        let output = shell(repo, command).output().unwrap();
        assert!(
            output.status.success(),
            "{manager}: {command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// The Provenance commands in the code spans of a Markdown text. A pipeline
/// gives the part that runs Provenance.
fn provenance_commands(markdown: &str) -> Vec<String> {
    Parser::new(markdown)
        .filter_map(|event| match event {
            Event::Code(code) => Some(code.into_string()),
            _ => None,
        })
        .flat_map(|code| code.split(" | ").map(str::to_owned).collect::<Vec<_>>())
        .filter(|part| part.split_whitespace().any(names_provenance_program))
        .collect()
}

fn names_provenance_program(word: &str) -> bool {
    matches!(
        word.trim_matches('"'),
        "provenance" | "./node_modules/.bin/provenance" | PACKAGE_BIN
    )
}

fn handoff_command(printed: &str) -> Option<String> {
    let line = printed
        .lines()
        .find(|line| line.starts_with("Have your agent run "))?;
    Some(
        line.strip_prefix("Have your agent run ")?
            .strip_suffix(" to get acclimated.")?
            .to_owned(),
    )
}
