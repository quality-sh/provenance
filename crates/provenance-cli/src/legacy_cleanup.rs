use crate::skills::stamp::{fnv1a64, header_hash};
use provenance_macros::rule;
use std::path::{Path, PathBuf};

pub const LEGACY_SKILL_DIRECTORIES: &[&str] = &[
    "shaping",
    "fork-tournament",
    "swarm-backtrace",
    "grounded-writing",
];
const BEGIN_MARKER: &str = "<!-- BEGIN PROVENANCE SKILLS -->";
const END_MARKER: &str = "<!-- END PROVENANCE SKILLS -->";
const AGENTS_PREAMBLE: &str = "# Provenance Skills\n\nThese skills are distributed with the provenance CLI and should match the installed binary.\n\nBefore shaping or backtrace work, run `provenance skills install --target agents-md` if skills are absent.\n";

pub fn skill_paths(base: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    [base.join(".claude/skills"), base.join(".agents/skills")]
        .into_iter()
        .flat_map(|root| {
            LEGACY_SKILL_DIRECTORIES
                .iter()
                .map(move |directory| root.join(directory).join("SKILL.md"))
        })
}

pub fn agents_path(base: &Path, global: bool) -> PathBuf {
    if global {
        base.join(".agents/AGENTS.md")
    } else {
        base.join("AGENTS.md")
    }
}

#[rule("rule_legacy_cleanup_ownership")]
pub fn valid_managed_skill(contents: &str) -> bool {
    let Some(frontmatter_end) = contents
        .strip_prefix("---\n")
        .and_then(|rest| rest.find("\n---\n"))
        .map(|end| end + "---\n".len() + "\n---\n".len())
    else {
        return false;
    };
    let Some(after_header) = contents[frontmatter_end..].find('\n') else {
        return false;
    };
    let header_end = frontmatter_end + after_header;
    let header = &contents[frontmatter_end..header_end];
    let payload = &contents[header_end + 1..];
    let installed = format!("{}{payload}", &contents[..frontmatter_end]);
    hash_proves_ownership(header, &installed)
}

fn hash_proves_ownership(header: &str, installed: &str) -> bool {
    header_hash(header) == Some(fnv1a64(installed).as_str())
}

pub fn project_agents(contents: &[u8]) -> Vec<u8> {
    let begin = BEGIN_MARKER.as_bytes();
    let end_marker = END_MARKER.as_bytes();
    let Some(start) = find_bytes(contents, begin) else {
        return contents.to_vec();
    };
    let block = &contents[start..];
    let Some(end_offset) = find_bytes(block, end_marker) else {
        return contents.to_vec();
    };
    let marker_line_end = BEGIN_MARKER.len() + 1;
    if !block.starts_with(format!("{BEGIN_MARKER}\n").as_bytes()) {
        return contents.to_vec();
    }
    let Some(header_line_end) = block[marker_line_end..]
        .iter()
        .position(|byte| *byte == b'\n')
    else {
        return contents.to_vec();
    };
    let header_end = marker_line_end + header_line_end;
    let Ok(header) = std::str::from_utf8(&block[marker_line_end..header_end]) else {
        return contents.to_vec();
    };
    let Ok(payload) = std::str::from_utf8(&block[header_end + 1..end_offset]) else {
        return contents.to_vec();
    };
    let Some(installed) = legacy_agents_source(payload) else {
        return contents.to_vec();
    };
    if !hash_proves_ownership(header, &installed) {
        return contents.to_vec();
    }
    let mut end = start + end_offset + END_MARKER.len();
    if contents.get(end) == Some(&b'\n') {
        end += 1;
    }
    let mut updated = contents[..start].to_vec();
    updated.extend_from_slice(&contents[end..]);
    updated
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn legacy_agents_source(payload: &str) -> Option<String> {
    let sections = payload.strip_prefix(AGENTS_PREAMBLE)?.strip_prefix('\n')?;
    let mut source = String::new();
    for section in sections.split("\n## Skill: ") {
        let section = section.strip_prefix("## Skill: ").unwrap_or(section);
        let (name, body) = section.split_once("\n\n")?;
        source.push_str("---\nname: ");
        source.push_str(name);
        source.push_str("\ndescription: ");
        source.push_str(legacy_description(name)?);
        source.push_str("\n---\n\n");
        source.push_str(body);
    }
    Some(source)
}

fn legacy_description(name: &str) -> Option<&'static str> {
    match name {
        "fork-tournament" => Some("Run a fork tournament when a shaping session hits a genuine design fork — mutually exclusive directions, expensive to reverse, and the human's preference unknowable without concrete artifacts to react to. Implements the `prototype` resolution method from docs/shaping.md - spawn stance-based agents producing competing artifacts as proposals (phase 1, end session), then present them for human disposal and land the decision as a Resolution (phase 2)."),
        "shaping" => Some("Guide turn-based requirement shaping in Provenance. Use when a user brings a loose idea, asks to refine requirements, work through open shaping questions, graduate fog, or run the Chart/Work loop against an anchor requirement. Land every resolved decision immediately into the graph."),
        "swarm-backtrace" => Some("Reverse-engineer candidate requirements from an existing codebase with a multi-agent swarm. Use when the user wants to extract, mine, backtrace, or reverse-engineer requirements or rules from existing code, bootstrap a Provenance graph from a legacy system, or asks \"what must be true for this code to be correct\". Lands everything as proposals (promotion_state=proposed) against a commit-pinned source — never as active requirements."),
        "grounded-writing" => Some("Write specific, evidence-grounded statements for requirements, rules, sources, resolutions, and boundaries — not generic capability language. Use before calling `requirements create/update`, `rules create/update`, `sources create/update`, `resolutions create/update`, or `boundaries create`, especially for a root or mid-level requirement, a statement merging several candidates, or a resolution's position and rationale."),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
