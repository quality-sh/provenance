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
- A Requirement or Rule description says how the product behaves, in the present
  tense: its states, what changes them, what the person sees, and one example.
  History goes on the record's thread. Unknowns go in fog or a Question.
- Before a graph write, run
  `{command} statement-checks create --statement "<statement>" --format json`. A clean report covers only the
  ASD-STE100 Issue 9 checks that Provenance implements. It does not prove full
  conformance.
- Plan: `{command} prime --quiet`
- New obligation: `{command} rules create --scope {selected_scope} --id rule_<slug> --requirement-id <req> --statement "<testable clause>"`
- Annotate implementation with `rule`, tests with `verifies`. Annotations move
  with code.
- Change a Requirement, Rule, or Resolution with a guarded record update.
  A content change opens a review submission unless a reviewer accepted that content.
  Git supplies record history and change evidence. Proposal, Disposition, Withdrawal,
  and Discussion records hold review state.
- When a session starts to write records, start or reuse the review host:
  `{command} review --repo . --repository-id local --scope {selected_scope} --detach --no-open`.
  One host serves the repository and scope for all sessions. It keeps running after
  the command returns, and later sessions reuse it, so leave it running. Give the
  person the `review_url` it prints, as plain text in chat. The page follows records
  as they land. A link works once and expires after 120 seconds. Run the command
  again for a new link.
- After you create or change a record, run
  `{command} <record-id> --scope {selected_scope} --review-link --format json` and give the
  person `review_url` as plain text in chat, whether or not `opened` is true. In an app
  that shows web pages, such as Claude Desktop or the Codex app, you can also run
  `{command} <record-id> --scope {selected_scope} --review-link --no-open --format json` and
  open `review_url` in the app. Opening the page is extra. Always give the link in chat
  too. Never give the person a record ID.
- The page is served on the machine that runs the host. If that is not the person's
  machine, say so, and give the person this command to run on their own machine:
  `{command} review --repo . --repository-id local --scope {selected_scope} --detach`.
  It opens the page in their browser.
- Read the decision and review feedback with `{command} <record-id> --scope {selected_scope} --view review`.
  Copy `review.edit.etag` unchanged to `--if-match` on the next guarded update.
  If feedback is truncated, use the continuation commands in the review view.
- Receive saved changes while you work with
  `{command} <requirement-id> --scope {selected_scope} --view review --follow --format json`.
  The command needs a running review host. It returns current state, not a delivery journal.
  A reset or disconnect means notification continuity is unknown.
- A guarded update after a rejection opens a new submission unless a reviewer
  accepted that content. To end a pending review without a replacement, withdraw it.
- A Question or Topic update needs the current etag. Read it with
  `{command} questions <question_id> get --scope {selected_scope} --format json` or
  `{command} topics <topic_id> get --scope {selected_scope} --format json` and pass
  `data.edit.etag` unchanged as `--if-match`.
- To drop a Question or Topic, archive it with its commit evidence. Archiving a
  Topic also archives its Questions. Discussion history stays readable:
  `printf '%s' '{"status":"archived","archived_in_commit":{"commit":"<full_commit_sha>"}}' | {command} questions <question_id> update --scope {selected_scope} --if-match <etag> --stdin --format json`
  `printf '%s' '{"status":"archived","archived_in_commit":{"commit":"<full_commit_sha>"}}' | {command} topics <topic_id> update --scope {selected_scope} --if-match <etag> --stdin --format json`
- Write graph state only through the Provenance CLI or SDK. Do not edit
  `.provenance/state` directly.
- Pre-commit: `{command} check --quiet` and
  `{command} coverage scan --path . --scope {selected_scope} --validate-rules`.
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

/// Reports whether the exact Provenance heading already owns a section, so a
/// summary can tell an added section from an updated one.
pub fn owns_section(existing: &str) -> bool {
    section_headings(existing)
        .iter()
        .any(|offset| line_at(existing, *offset) == HEADING)
}

/// Selects a declared scope before init plans any project writes.
pub fn guidance_scope(
    path: &camino::Utf8Path,
    explicit: Option<&str>,
    manifest: &provenance_core::Manifest,
) -> anyhow::Result<String> {
    if let Some(scope) = explicit {
        return Ok(scope.to_owned());
    }
    if let [only] = manifest.scopes.as_slice() {
        return Ok(only.id.as_str().to_owned());
    }
    let agents = crate::atomic_file::FileSnapshot::read(path.join("AGENTS.md").as_std_path())?;
    let text = std::str::from_utf8(agents.bytes().unwrap_or_default())
        .context("AGENTS.md is not valid UTF-8")?;
    let headings = section_headings(text);
    if let Some(index) = headings
        .iter()
        .position(|offset| line_at(text, *offset) == HEADING)
    {
        let end = headings.get(index + 1).copied().unwrap_or(text.len());
        let section = &text[headings[index]..end];
        if let Some(scope) = section
            .split("--scope ")
            .nth(1)
            .and_then(|tail| tail.split_whitespace().next())
        {
            if manifest.scopes.iter().any(|item| item.id.as_str() == scope) {
                return Ok(scope.to_owned());
            }
        }
    }
    if manifest
        .scopes
        .iter()
        .any(|item| item.id.as_str() == "default")
    {
        return Ok("default".to_owned());
    }
    let scopes = manifest
        .scopes
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::bail!("cannot select an init scope; declared scopes: {scopes}; run init again with --scope <scope>")
}

/// Projects only the instruction section owned by the exact Provenance heading.
#[rule("rule_init_owns_agents_provenance_section")]
pub fn project(existing: &[u8], invocation: &Invocation, scope: &str) -> anyhow::Result<Vec<u8>> {
    let existing = std::str::from_utf8(existing).context("AGENTS.md is not valid UTF-8")?;
    let instructions = instructions(invocation, scope);
    Ok(update_agents(existing, &instructions).into_bytes())
}

/// Writes channel-local commands and the required statement-authoring safeguards.
#[rule("rule_init_typescript_local_command")]
#[rule("rule_init_native_command")]
#[rule("rule_init_grounded_writing_guidance")]
#[rule("rule_init_statement_preflight_guidance")]
#[rule("rule_init_statement_claim_limit")]
#[rule("rule_init_canonical_write_path")]
#[rule("rule_init_guidance_selected_scope")]
#[rule("rule_init_review_host_guidance")]
#[rule("rule_init_description_guidance")]
#[rule("rule_init_guidance_background_review_host")]
fn instructions(invocation: &Invocation, scope: &str) -> String {
    INSTRUCTIONS
        .replace("{command}", invocation.0)
        .replace("{selected_scope}", scope)
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
