use anyhow::Context;
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

/// Projects only the instruction section owned by the exact Provenance heading.
#[rule("rule_init_installs_bundled_skills")]
#[rule("rule_init_owns_agents_provenance_section")]
pub fn project(existing: &[u8]) -> anyhow::Result<Vec<u8>> {
    let existing = std::str::from_utf8(existing).context("AGENTS.md is not valid UTF-8")?;
    Ok(update_agents(existing).into_bytes())
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
