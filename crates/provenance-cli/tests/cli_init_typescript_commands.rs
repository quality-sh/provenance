//! TypeScript initialization for each supported package manager: each command
//! that init prints or writes in AGENTS.md uses the project package, and the
//! package manager gives each argument to Provenance.

use provenance_macros::verifies;
use pulldown_cmark::{Event, Parser};
use std::path::{Path, PathBuf};
use std::process::Output;

/// The package manager, the executable that the test looks for on PATH, and
/// the command that runs the project package through that package manager.
const MANAGERS: [(&str, &str, &str); 6] = [
    ("npm", "npx", "npx --no -- provenance"),
    ("pnpm", "pnpm", "pnpm exec ./node_modules/.bin/provenance"),
    ("yarn", "yarn", "yarn run -B provenance"),
    ("bun", "bunx", "bunx --no-install provenance"),
    (
        "deno",
        "deno",
        "deno task --node-modules-dir=manual --eval \"./node_modules/.bin/provenance\"",
    ),
    ("nub", "nub", "nub exec provenance"),
];

/// Yarn and Nub find a binary only through a real package installation, so
/// this test does not run their commands.
const RUN_WITH_PROJECT_BINARY: [&str; 4] = ["npm", "pnpm", "bun", "deno"];

#[test]
#[verifies("rule_init_typescript_local_command", examples)]
fn typescript_init_prints_and_writes_package_manager_commands() {
    for (index, (manager, executable, prefix)) in MANAGERS.into_iter().enumerate() {
        let temporary = tempfile::tempdir().unwrap();
        let repo = temporary.path().join("repo");
        let output = init_typescript(temporary.path(), &repo, manager, index % 2 == 0);
        assert!(
            output.status.success(),
            "{manager}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let printed = format!(
            "{}{}",
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap()
        );
        let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
        let section = agents.split_once("## Provenance\n").unwrap().1;

        let mut commands = provenance_commands(&printed);
        commands.extend(handoff_command(&printed));
        let printed_count = commands.len();
        commands.extend(provenance_commands(section));
        for command in &commands {
            assert!(
                command.starts_with(&format!("{prefix} ")),
                "{manager}: {command}"
            );
            let address = command[prefix.len()..].split_whitespace().next();
            assert!(
                !matches!(
                    address,
                    Some("rules" | "requirements" | "questions" | "topics")
                ),
                "{manager}: {command}"
            );
        }
        let expected_printed = [
            format!("{prefix} prime"),
            format!("{prefix} dictionary import --pdf <path>"),
            format!(
                "{prefix} init --path {} --disposition-actor-id <reviewer-id>",
                repo.display()
            ),
        ];
        for expected in &expected_printed {
            assert!(
                commands[..printed_count].contains(expected),
                "{manager}: {expected}"
            );
        }
        assert_eq!(
            printed_count,
            expected_printed.len(),
            "{manager}: {printed}"
        );
        for expected in ["--version", "prime --quiet", "check --quiet"] {
            assert!(
                commands.contains(&format!("{prefix} {expected}")),
                "{manager}: {expected}"
            );
        }
        assert!(section.contains(&format!("replace that word with `{prefix}`")));

        if cfg!(unix) && RUN_WITH_PROJECT_BINARY.contains(&manager) && on_path(executable) {
            run_commands(&repo, manager, prefix, &commands);
        }
    }
}

fn init_typescript(temporary: &Path, repo: &Path, manager: &str, git_identity: bool) -> Output {
    std::fs::create_dir_all(repo.join("node_modules/.bin")).unwrap();
    std::fs::write(
        repo.join("package.json"),
        "{\"name\":\"project\",\"version\":\"1.0.0\",\"private\":true}\n",
    )
    .unwrap();
    link_project_binary(&repo.join("node_modules/.bin/provenance"));
    let git_config = temporary.join("gitconfig");
    let identity = if git_identity {
        "[user]\n\temail = reviewer@example.com\n"
    } else {
        ""
    };
    std::fs::write(&git_config, identity).unwrap();
    let git = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", &git_config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(git.success());
    std::process::Command::new(provenance_binary())
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .args(["--path-prefix", "."])
        .args(["--invocation-channel", "typescript"])
        .args(["--package-manager", manager])
        .env("GIT_CONFIG_GLOBAL", &git_config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("PROVENANCE_STE100_ASSET_DIR", temporary.join("assets"))
        .env("PROVENANCE_STE100_INDEX_DIR", temporary.join("indexes"))
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            env!("PROVENANCE_TEST_STE100_ASSET_URL"),
        )
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}

/// Runs the version and help commands, and each written command that has no
/// placeholder and does not start a review host, with the package manager.
fn run_commands(repo: &Path, manager: &str, prefix: &str, commands: &[String]) {
    let version = shell(repo, &format!("{prefix} --version"));
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        format!("provenance {}\n", env!("CARGO_PKG_VERSION")),
        "{manager}"
    );
    let help = shell(repo, &format!("{prefix} --help"));
    assert!(
        String::from_utf8_lossy(&help.stdout).contains("Usage: provenance"),
        "{manager}: {}",
        String::from_utf8_lossy(&help.stdout)
    );
    for command in commands {
        if command.contains('<') || command.contains(" review ") {
            continue;
        }
        let output = shell(repo, command);
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
        "provenance" | "./node_modules/.bin/provenance"
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

fn on_path(executable: &str) -> bool {
    std::process::Command::new(executable)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn shell(repo: &Path, command: &str) -> Output {
    std::process::Command::new("sh")
        .args(["-c", command])
        .current_dir(repo)
        .env("npm_config_update_notifier", "false")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}

fn provenance_binary() -> PathBuf {
    PathBuf::from(assert_cmd::cargo::cargo_bin!("provenance"))
}

/// Puts the CLI under test where the package manager installs the package binary.
#[cfg(unix)]
fn link_project_binary(path: &Path) {
    std::os::unix::fs::symlink(provenance_binary(), path).unwrap();
}

/// This host does not run the commands, so the project needs no binary.
#[cfg(not(unix))]
fn link_project_binary(_path: &Path) {}
