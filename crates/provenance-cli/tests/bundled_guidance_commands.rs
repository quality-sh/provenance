use assert_cmd::Command;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use std::collections::BTreeSet;
use std::path::Path;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

#[test]
fn installed_agent_guidance_names_only_supported_cli_commands_and_options() {
    let directory = tempfile::tempdir().expect("create temporary repository");
    let repo = directory.path().to_string_lossy().into_owned();
    let initialized = provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .output()
        .expect("initialize repository");
    assert!(
        initialized.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&initialized.stderr)
    );

    let mut guidance = vec![
        std::fs::read_to_string(directory.path().join("AGENTS.md"))
            .expect("read installed AGENTS.md"),
        String::from_utf8(initialized.stdout).expect("init output is UTF-8"),
    ];
    collect_skill_text(directory.path(), &mut guidance);
    let prime = provenance()
        .args(["prime", "--repo", &repo])
        .output()
        .expect("read prime guidance");
    assert!(prime.status.success(), "prime command succeeds");
    guidance.push(String::from_utf8(prime.stdout).expect("prime output is UTF-8"));

    let commands = guidance
        .iter()
        .flat_map(|text| markdown_commands(text))
        .collect::<BTreeSet<_>>();
    assert!(!commands.is_empty(), "installed guidance contains commands");

    let mut failures = Vec::new();
    for command in commands {
        if let Err(error) = check_help(&command, directory.path()) {
            failures.push(format!("{command}: {error}"));
        }
    }
    assert!(
        failures.is_empty(),
        "bundled guidance contains unsupported CLI syntax:\n{}",
        failures.join("\n")
    );
}

fn collect_skill_text(repo: &Path, guidance: &mut Vec<String>) {
    let skills = repo.join(".agents/skills");
    for entry in std::fs::read_dir(skills).expect("read installed skills") {
        let path = entry.expect("read skill entry").path().join("SKILL.md");
        guidance.push(std::fs::read_to_string(path).expect("read installed skill"));
    }
}

fn markdown_commands(markdown: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut block = None;
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language)))
                if matches!(language.as_ref(), "sh" | "bash" | "shell") =>
            {
                block = Some(String::new());
            }
            Event::Text(text) if block.is_some() => block.as_mut().unwrap().push_str(&text),
            Event::End(TagEnd::CodeBlock) if block.is_some() => {
                commands.extend(shell_block_commands(&block.take().unwrap()));
            }
            Event::Code(code) => {
                commands.extend(command_fragments(&code));
            }
            Event::Text(text) => commands.extend(command_fragments(&text)),
            _ => {}
        }
    }
    commands
}

fn shell_block_commands(block: &str) -> Vec<String> {
    let mut logical = String::new();
    let mut commands = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        logical.push_str(trimmed.trim_end_matches('\\'));
        if trimmed.ends_with('\\') {
            logical.push(' ');
        } else {
            commands.extend(command_fragments(&logical));
            logical.clear();
        }
    }
    commands
}

fn command_fragments(text: &str) -> Vec<String> {
    text.split(['\n', ';'])
        .filter_map(|fragment| {
            let start = fragment.find("provenance ")?;
            let command = fragment[start..]
                .trim()
                .trim_end_matches(['.', ',', ':', ')', '`']);
            Some(command.split_whitespace().collect::<Vec<_>>().join(" "))
        })
        .collect()
}

fn check_help(command: &str, repo: &Path) -> Result<(), String> {
    let words = command.split_whitespace().collect::<Vec<_>>();
    let option = words.iter().position(|word| word.starts_with("--"));
    let address_end = option.unwrap_or(words.len());
    let address = words
        .get(1..address_end)
        .ok_or_else(|| "command has no address".to_owned())?;
    let options = words
        .iter()
        .filter(|word| word.starts_with("--"))
        .map(|word| word.trim_end_matches(['.', ',', ')', '`']))
        .collect::<BTreeSet<_>>();
    let output = provenance()
        .current_dir(repo)
        .args(address)
        .arg("--help")
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let help = String::from_utf8_lossy(&output.stdout);
    for option in options {
        if !help.contains(option) {
            return Err(format!("option {option} is absent from operation help"));
        }
    }
    Ok(())
}
