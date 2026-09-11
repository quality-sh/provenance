//! Recognizes rule and verification binding sites, one site at a time.
//!
//! Rust binds through `#[rule]`/`#[verifies]` attributes (plain, qualified,
//! or wrapped across lines by rustfmt), Python through `@rule(...)`
//! decorators, and JS/TS/Go/Java through `rule(...)` and `verifies(...)`
//! calls found by the binding lexer.

use std::str::FromStr;

use super::Language;
use crate::binding_lexer::{call_arguments, free_call_arguments};
use crate::parser::Verification;

/// A recognized binding site: the rule id, the verification method for a
/// `verifies` site, and how many lines past the first the site consumed (a
/// wrapped Rust attribute spans several lines; every other site is one).
pub(super) struct ParsedBinding {
    pub rule_id: String,
    pub verification: Option<Verification>,
    pub extra_lines: usize,
}

pub(super) fn parse_binding_line(
    language: Language,
    line: &str,
    following: &[&str],
    in_block_comment: bool,
) -> Option<ParsedBinding> {
    match language {
        Language::Rust => (!in_block_comment)
            .then(|| parse_attribute(line, following))
            .flatten(),
        Language::Python => {
            parse_python_decorator(line).map(|(rule_id, verification)| ParsedBinding {
                rule_id,
                verification,
                extra_lines: 0,
            })
        }
        Language::JavaScript | Language::TypeScript => parse_script_call(line, in_block_comment),
        Language::Go | Language::Java => parse_rule_call(line, in_block_comment),
    }
}

const fn one_line(rule_id: String, verification: Option<Verification>) -> ParsedBinding {
    ParsedBinding {
        rule_id,
        verification,
        extra_lines: 0,
    }
}

fn parse_python_decorator(line: &str) -> Option<(String, Option<Verification>)> {
    let trimmed = line.trim_start();
    let decorator = trimmed.strip_prefix('@')?;
    let rest = decorator.strip_prefix("rule(").or_else(|| {
        decorator
            .split_once(".rule(")
            .filter(|(qualifier, _)| !qualifier.is_empty())
            .map(|(_, rest)| rest)
    })?;
    Some((quoted_literal(rest)?.0, None))
}

fn parse_script_call(line: &str, in_block_comment: bool) -> Option<ParsedBinding> {
    if let Some(rest) = free_call_arguments(line, in_block_comment, "verifies") {
        let (rule_id, after_id) = quoted_literal(rest)?;
        let method = argument_after_comma(after_id)?;
        return Some(one_line(
            rule_id,
            Some(Verification::from_str(method).ok()?),
        ));
    }
    // A `receiver.rule("id", ...)` call declares a Rule in the SDK's test
    // graph; only a free `rule("id", implementation)` call binds production
    // code to that Rule.
    let rest = free_call_arguments(line, in_block_comment, "rule")?;
    let (rule_id, after_id) = quoted_literal(rest)?;
    after_id
        .trim_start()
        .starts_with(',')
        .then(|| one_line(rule_id, None))
}

fn parse_rule_call(line: &str, in_block_comment: bool) -> Option<ParsedBinding> {
    let rest = call_arguments(line, in_block_comment, "rule")?;
    let (rule_id, after_id) = quoted_literal(rest)?;
    after_id
        .trim_start()
        .starts_with(',')
        .then(|| one_line(rule_id, None))
}

fn quoted_literal(rest: &str) -> Option<(String, &str)> {
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if !matches!(quote, '\'' | '"' | '`') {
        return None;
    }
    let after_quote = &rest[quote.len_utf8()..];
    let end = after_quote.find(quote)?;
    let literal = &after_quote[..end];
    if literal.is_empty() {
        return None;
    }
    Some((literal.to_string(), &after_quote[end + quote.len_utf8()..]))
}

fn argument_after_comma(rest: &str) -> Option<&str> {
    let argument = rest.trim_start().strip_prefix(',')?.trim_start();
    let unquoted = argument.strip_prefix(['\'', '"', '`']).unwrap_or(argument);
    let method = unquoted
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()?;
    (!method.is_empty()).then_some(method)
}

/// Recognizes `#[rule("id")]` and `#[verifies("id", method)]` attributes,
/// plain or `path::`-qualified, on one line or wrapped across lines the way
/// rustfmt formats long arguments. `line` is the current line outside
/// multiline strings; `following` holds the raw lines after it. The proc
/// macros reject malformed arguments at compile time, so anything found in
/// compiling code is well-formed; lines that do not match are silently
/// skipped.
fn parse_attribute(line: &str, following: &[&str]) -> Option<ParsedBinding> {
    let (joined, extra_lines) = join_attribute(line, following)?;
    parse_attribute_text(&joined).map(|(rule_id, verification)| ParsedBinding {
        rule_id,
        verification,
        extra_lines,
    })
}

/// The attribute name a `#[` line opens with, when it is one the scanner
/// binds: `rule` or `verifies`, optionally qualified (`provenance_macros::rule`).
fn binding_attribute_name(line: &str) -> Option<&str> {
    let inner = line.trim_start().strip_prefix("#[")?;
    let (name, _) = inner.split_once('(')?;
    let name = name.trim_end();
    (name == "rule"
        || name == "verifies"
        || name.ends_with("::rule")
        || name.ends_with("::verifies"))
    .then_some(name)
}

/// Joins a wrapped attribute into one logical line. Returns `None` unless
/// the opening line names a binding attribute and the parentheses close
/// within a small fixed window. Comments never appear inside a well-formed
/// wrapped attribute; one that would pull comment text in is left unparsed.
fn join_attribute(line: &str, following: &[&str]) -> Option<(String, usize)> {
    binding_attribute_name(line)?;
    let mut joined = line.trim().to_string();
    let mut extra = 0;
    while paren_depth(&joined) > 0 {
        let next = following.get(extra)?.trim();
        if next.contains("//") || next.contains("/*") {
            return None;
        }
        joined.push(' ');
        joined.push_str(next);
        extra += 1;
        if extra > MAX_ATTRIBUTE_LINES {
            return None;
        }
    }
    Some((joined, extra))
}

/// A wrapped attribute never legitimately spans more than a few lines.
const MAX_ATTRIBUTE_LINES: usize = 16;

fn paren_depth(text: &str) -> usize {
    let opens = text.matches('(').count();
    let closes = text.matches(')').count();
    opens.saturating_sub(closes)
}

fn parse_attribute_text(joined: &str) -> Option<(String, Option<Verification>)> {
    let arguments = joined
        .trim_end()
        .strip_suffix(']')?
        .strip_suffix(')')?
        .strip_prefix("#[")?
        .split_once('(')?
        .1;
    let name = binding_attribute_name(joined)?;
    if name == "rule" || name.ends_with("::rule") {
        return Some((string_literal(arguments.trim_start())?, None));
    }
    let (id, method) = arguments.split_once(',')?;
    let method = method.trim();
    Some((
        string_literal(id.trim_start())?,
        Some(Verification::from_str(method).ok()?),
    ))
}

fn string_literal(rest: &str) -> Option<String> {
    let after_quote = rest.strip_prefix('"')?;
    let (literal, _) = after_quote.split_once('"')?;
    (!literal.is_empty()).then(|| literal.to_string())
}
