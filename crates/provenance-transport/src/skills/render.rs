//! Turns an embedded skill into the file provenance writes to disk: the
//! skill's own text with the ownership stamp inserted straight after its
//! frontmatter.

use super::stamp;
use super::EmbeddedSkill;

const FRONTMATTER_START: &str = "---\n";
const FRONTMATTER_END: &str = "\n---\n";

/// The installed form of `skill`: its text with the stamp on the first line
/// after the frontmatter, or - for a skill without frontmatter - on the first
/// line of the file. The stamp covers everything else in the file, so
/// `legacy_cleanup` can hash the file back with the stamp line removed.
pub fn skill_file(skill: &EmbeddedSkill) -> String {
    let header = stamp::header(skill.content);
    if let Some(rest) = skill.content.strip_prefix(FRONTMATTER_START) {
        if let Some(end) = rest.find(FRONTMATTER_END) {
            let insertion = FRONTMATTER_START.len() + end + FRONTMATTER_END.len();
            return format!(
                "{}{}\n{}",
                &skill.content[..insertion],
                header,
                &skill.content[insertion..]
            );
        }
    }
    format!("{header}\n{}", skill.content)
}

/// The value of `field` in the skill's frontmatter, or `None` when the file
/// has no frontmatter or the block does not carry the field.
pub fn frontmatter_field<'a>(content: &'a str, field: &str) -> Option<&'a str> {
    let mut lines = content.lines();
    if lines.next()? != "---" {
        return None;
    }
    let prefix = format!("{field}:");
    for line in lines {
        if line == "---" {
            return None;
        }
        if let Some(value) = line.strip_prefix(&prefix) {
            return Some(value.trim());
        }
    }
    None
}
