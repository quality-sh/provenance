use anyhow::Context;
use camino::Utf8Path;
use provenance_macros::rule;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

const HEADING: &str = "## Provenance";
const INSTRUCTIONS: &str = r#"## Provenance

Requirements live in a Provenance graph. Plan changes with the graph and update
it in the same change.

- Plan: `provenance prime --quiet`
- New obligation: `provenance rules create --scope default --id rule_<slug> --requirement-id <req> --statement "<testable clause>"`
- Annotate implementation with `rule`, tests with `verifies`. Annotations move
  with code.
- To change a Requirement, Rule, or past decision, create a Proposal. A human decides each
  Proposal.
- Pre-commit: `provenance check --quiet` and
  `provenance coverage scan --path . --scope default --validate-rules`.
  Commit graph updates with the code."#;

/// Installs the bundled skills and repository instructions for every init path.
#[rule("rule_init_installs_bundled_skills")]
pub fn install(repo: &Utf8Path) -> anyhow::Result<()> {
    crate::skills::install_at(repo.as_std_path(), false, false, false)
        .context("failed to install the bundled Provenance skills")?;
    inject_agents_instructions(repo)
}

/// Writes only the instruction section owned by the exact Provenance heading.
#[rule("rule_init_owns_agents_provenance_section")]
fn inject_agents_instructions(repo: &Utf8Path) -> anyhow::Result<()> {
    let path = repo.join("AGENTS.md");
    let existing = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).with_context(|| format!("failed to read {path}")),
    };
    let updated = update_agents(&existing);
    if updated != existing {
        std::fs::write(&path, updated).with_context(|| format!("failed to write {path}"))?;
    }
    Ok(())
}

fn update_agents(existing: &str) -> String {
    let headings = section_headings(existing);
    let Some(index) = headings
        .iter()
        .position(|offset| line_at(existing, *offset) == HEADING)
    else {
        return append_instructions(existing);
    };
    let start = headings[index];
    let end = headings.get(index + 1).copied().unwrap_or(existing.len());

    let mut updated = String::with_capacity(existing.len() + INSTRUCTIONS.len());
    updated.push_str(&existing[..start]);
    updated.push_str(INSTRUCTIONS);
    if end == existing.len() {
        updated.push('\n');
    } else {
        updated.push_str("\n\n");
        updated.push_str(&existing[end..]);
    }
    updated
}

fn section_headings(text: &str) -> Vec<usize> {
    Parser::new(text)
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1 | HeadingLevel::H2,
                ..
            }) if range.start == 0 || text.as_bytes()[range.start - 1] == b'\n' => {
                Some(range.start)
            }
            _ => None,
        })
        .collect()
}

fn append_instructions(existing: &str) -> String {
    let mut updated = String::with_capacity(existing.len() + INSTRUCTIONS.len() + 2);
    updated.push_str(existing);
    if !existing.is_empty() {
        if !existing.ends_with('\n') {
            updated.push('\n');
        }
        if !updated.ends_with("\n\n") {
            updated.push('\n');
        }
    }
    updated.push_str(INSTRUCTIONS);
    updated.push('\n');
    updated
}

fn line_at(text: &str, start: usize) -> &str {
    text[start..]
        .split_once('\n')
        .map_or(&text[start..], |(line, _)| line)
        .trim_end_matches('\r')
}
