//! Text safety for untrusted envelope text.
//!
//! Untrusted text is data. It never starts a report line, so it cannot
//! create headings, lists, fences or workflow commands at line start. The
//! transformations below keep the text readable while making markup
//! impossible: HTML entities stay visible, links and images lose their
//! brackets, code fences lose their backticks, mentions lose their trigger,
//! and table cells keep their column count.

use provenance_macros::rule;

/// Maximum characters of untrusted text rendered in one place. Longer text
/// is cut on a character boundary and the cut is stated in the output.
pub const MAX_TEXT_CHARS: usize = 200;

struct Truncated {
    text: String,
    omitted: Option<usize>,
}

/// Cut to the character budget before any transformation, so an entity or
/// escape sequence is never cut in half.
fn truncate_chars(text: &str, max: usize) -> Truncated {
    let len = text.chars().count();
    if len <= max {
        return Truncated {
            text: text.to_string(),
            omitted: None,
        };
    }
    let cut: String = text.chars().take(max).collect();
    Truncated {
        text: cut,
        omitted: Some(len - max),
    }
}

/// Reduce one piece of untrusted text to safe inline text.
#[rule("rule_report_escapes_untrusted_text")]
pub fn escape_inline(text: &str) -> String {
    let truncated = truncate_chars(text, MAX_TEXT_CHARS);
    let mut out = String::new();
    for ch in truncated.text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '`' => out.push('\''),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '@' => {
                out.push('@');
                out.push('\u{200B}');
            }
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    if let Some(omitted) = truncated.omitted {
        out.push_str(" [truncated; ");
        out.push_str(&omitted.to_string());
        out.push_str(" characters omitted]");
    }
    out
}

/// Reduce one piece of untrusted text to one safe table cell. Newlines never
/// survive, and delimiters are escaped so a cell cannot add a column.
pub fn escape_cell(text: &str) -> String {
    escape_inline(&text.replace(['\n', '\r'], " ")).replace('|', "\\|")
}
