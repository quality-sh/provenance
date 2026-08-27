use crate::cli::{InvocationChannel, PackageManager};
use anyhow::Context;
use provenance_macros::rule;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

const HEADING: &str = "## Provenance";
const INSTRUCTIONS: &str = r#"## Provenance

Requirements live in a Provenance graph. Plan changes with the graph and update
it in the same change.

- Use the `provenance-grounded-writing` skill before you write or change a
  Requirement or Rule statement.
- Before a graph write, send `{"statement":"<statement>"}` to
  `{command} sdk check-statement --format json`. A clean report covers only the
  ASD-STE100 Issue 9 checks that Provenance implements. It does not prove full
  conformance.
- Plan: `{command} prime --quiet`
- New obligation: `{command} rules create --scope default --id rule_<slug> --requirement-id <req> --statement "<testable clause>"`
- Annotate implementation with `rule`, tests with `verifies`. Annotations move
  with code.
- To change a Requirement, Rule, or past decision, create a Proposal. A human decides each
  Proposal.
- Write graph state only through the Provenance CLI or SDK. Do not edit
  `.provenance/state` directly.
- Pre-commit: `{command} check --quiet` and
  `{command} coverage scan --path . --scope default --validate-rules`.
  Commit graph updates with the code.
- ASD owns ASD-STE100. STEMG maintains it. Use the official Issue 9 request page:
  https://www.asd-ste100.org/STE_downloads.html#article02-2l. Provenance names
  only its implemented checks and makes no compliance or endorsement claim."#;

pub struct Invocation(&'static str);

impl Invocation {
    pub(super) fn from_cli(
        channel: InvocationChannel,
        package_manager: Option<PackageManager>,
    ) -> anyhow::Result<Self> {
        match (channel, package_manager) {
            (InvocationChannel::Native, None) => Ok(Self("provenance")),
            (InvocationChannel::Native, Some(_)) => {
                anyhow::bail!("--package-manager requires the TypeScript invocation channel")
            }
            (InvocationChannel::Typescript, None) => {
                anyhow::bail!("TypeScript initialization requires --package-manager")
            }
            (InvocationChannel::Typescript, Some(manager)) => Ok(Self(match manager {
                PackageManager::Npm => "npx --no provenance",
                PackageManager::Pnpm => "pnpm exec ./node_modules/.bin/provenance",
                PackageManager::Yarn => "yarn run -B provenance",
                PackageManager::Bun => "bunx --no-install provenance",
                PackageManager::Deno => {
                    "deno task --node-modules-dir=manual --eval \"./node_modules/.bin/provenance\""
                }
                PackageManager::Nub => "nub exec provenance",
            })),
        }
    }
}

/// Projects only the instruction section owned by the exact Provenance heading.
#[rule("rule_init_owns_agents_provenance_section")]
pub fn project(existing: &[u8], invocation: &Invocation) -> anyhow::Result<Vec<u8>> {
    let existing = std::str::from_utf8(existing).context("AGENTS.md is not valid UTF-8")?;
    let instructions = instructions(invocation);
    Ok(update_agents(existing, &instructions).into_bytes())
}

/// Writes channel-local commands and the required statement-authoring safeguards.
#[rule("rule_init_typescript_local_command")]
#[rule("rule_init_native_command")]
#[rule("rule_init_grounded_writing_guidance")]
#[rule("rule_init_statement_preflight_guidance")]
#[rule("rule_init_statement_claim_limit")]
#[rule("rule_init_canonical_write_path")]
fn instructions(invocation: &Invocation) -> String {
    INSTRUCTIONS.replace("{command}", invocation.0)
}

fn update_agents(existing: &str, instructions: &str) -> String {
    let headings = section_headings(existing);
    let Some(index) = headings
        .iter()
        .position(|offset| line_at(existing, *offset) == HEADING)
    else {
        return append_instructions(existing, instructions);
    };
    let start = headings[index];
    let end = headings.get(index + 1).copied().unwrap_or(existing.len());

    let mut updated = String::with_capacity(existing.len() + instructions.len());
    updated.push_str(&existing[..start]);
    updated.push_str(instructions);
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

fn append_instructions(existing: &str, instructions: &str) -> String {
    let mut updated = String::with_capacity(existing.len() + instructions.len() + 2);
    updated.push_str(existing);
    if !existing.is_empty() {
        if !existing.ends_with('\n') {
            updated.push('\n');
        }
        if !updated.ends_with("\n\n") {
            updated.push('\n');
        }
    }
    updated.push_str(instructions);
    updated.push('\n');
    updated
}

fn line_at(text: &str, start: usize) -> &str {
    text[start..]
        .split_once('\n')
        .map_or(&text[start..], |(line, _)| line)
        .trim_end_matches('\r')
}
