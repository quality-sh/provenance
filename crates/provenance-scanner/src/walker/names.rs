//! The item names of the sites of one file, given after the walk, when the
//! line of each site is known.
//!
//! The scanner lexes the file one time. The read of a comment, attribute, or
//! decorator site covers the code tokens below the site, up to the next such
//! site. When those tokens hold only attributes, decorators, or annotations,
//! the declaration is below the next site, and the site takes the name that
//! the next site gets. A site that does not start a statement gets no name.
//! So each site above one declaration gets the name of that declaration, and
//! a site above an attribute whose arguments hold a site gets no name. The
//! reads do not overlap, so the sites of a file read each token at most one
//! time.

use std::ops::Range;

use provenance_core::coverage::EvidenceAnchor;

use super::declaration::{self, file_tokens, statement_starts, Read, Token};
use super::rust_lines::RustLexicalState;
use super::{script_call_name, AnnotationLocation, AttributeBinding};
use crate::Language;

/// The sites of one file.
pub(super) struct Sites<'a> {
    language: Language,
    lines: &'a [&'a str],
    original_lines: &'a [&'a str],
    unnamed: Vec<Unnamed>,
}

struct Unnamed {
    line: usize,
    /// The index of the first line that can hold the item of the site.
    below: usize,
    site: Site,
}

enum Site {
    /// The annotations of one comment, at these indexes.
    Comment(Range<usize>),
    /// The attribute or the decorator at this index of the bindings.
    Attribute(usize),
    /// The call at this index of the bindings.
    Call(usize),
}

impl<'a> Sites<'a> {
    /// The sites of the file whose lexical lines are `lines`.
    pub(super) const fn new(
        language: Language,
        lines: &'a [&'a str],
        original_lines: &'a [&'a str],
    ) -> Self {
        Self {
            language,
            lines,
            original_lines,
            unnamed: Vec::new(),
        }
    }

    /// A comment on line `line` that holds the annotations at `annotations`,
    /// with its item on line `below` or a line below it.
    pub(super) fn comment(&mut self, line: usize, below: usize, annotations: Range<usize>) {
        self.unnamed.push(Unnamed {
            line,
            below,
            site: Site::Comment(annotations),
        });
    }

    /// An attribute, a decorator, or a call from `line` to `last`, at the
    /// index `binding` of the bindings of the file.
    pub(super) fn binding(&mut self, line: usize, last: usize, binding: usize) {
        let site = if matches!(self.language, Language::Rust | Language::Python) {
            Site::Attribute(binding)
        } else {
            Site::Call(binding)
        };
        self.unnamed.push(Unnamed {
            line,
            below: last + 1,
            site,
        });
    }

    /// Gives each site its item name and the anchor of that name. A call
    /// takes its name by the rules of `script_call_name`.
    pub(super) fn name(
        self,
        rust_states: &[RustLexicalState],
        annotations: &mut [AnnotationLocation],
        bindings: &mut [AttributeBinding],
    ) {
        let Self {
            language,
            lines,
            original_lines,
            unnamed: sites,
        } = self;
        let tokens = file_tokens(language, lines, rust_states);
        let (calls, declared): (Vec<_>, Vec<_>) = sites
            .into_iter()
            .partition(|unnamed| matches!(unnamed.site, Site::Call(_)));
        let site_lines = declared
            .iter()
            .map(|unnamed| unnamed.line)
            .collect::<Vec<_>>();
        let starts = statement_starts(language, &tokens, &site_lines);
        let bounds = declared
            .iter()
            .map(|unnamed| (unnamed.below, unnamed.line))
            .collect::<Vec<_>>();
        let ranges = read_ranges(&tokens, &bounds);
        let mut next_name = None;
        let mut names = vec![None; declared.len()];
        for index in (0..declared.len()).rev() {
            let read = declaration::item_name(language, &tokens[ranges[index].clone()]);
            let name = match read {
                Read::Declaration(name) => name,
                Read::NextSite => next_name.take(),
            };
            names[index] = name.filter(|_| starts[index]);
            next_name.clone_from(&names[index]);
        }
        for (unnamed, name) in declared.into_iter().zip(names) {
            set(original_lines, &unnamed.site, name, annotations, bindings);
        }
        for unnamed in calls {
            if let Site::Call(index) = unnamed.site {
                let verification = bindings[index].verification;
                let name = script_call_name(lines, unnamed.below - 1, verification);
                set(original_lines, &unnamed.site, name, annotations, bindings);
            }
        }
    }
}

/// Gives the site `site` the item name `name` and the anchor of that name.
fn set(
    original_lines: &[&str],
    site: &Site,
    name: Option<String>,
    annotations: &mut [AnnotationLocation],
    bindings: &mut [AttributeBinding],
) {
    match site {
        Site::Comment(range) => {
            for annotation in &mut annotations[range.clone()] {
                let line = original_lines[annotation.line - 1];
                annotation.anchor = EvidenceAnchor::new(name.clone(), line);
                annotation.function_name.clone_from(&name);
            }
        }
        Site::Attribute(index) | Site::Call(index) => {
            let binding = &mut bindings[*index];
            let line = original_lines[binding.line - 1];
            binding.anchor = EvidenceAnchor::new(name.clone(), line);
            binding.item_name = name;
        }
    }
}

/// The range of the tokens that the read of each site covers. `sites` holds
/// the first line below each site and the line of the site, in line order. A
/// read covers the tokens from the first line below its site to the line of
/// the next site, so the ranges do not overlap.
fn read_ranges(tokens: &[Token<'_>], sites: &[(usize, usize)]) -> Vec<Range<usize>> {
    let first_at = |line: usize| tokens.partition_point(|token| token.line < line);
    sites
        .iter()
        .enumerate()
        .map(|(index, (below, _))| {
            let end = sites
                .get(index + 1)
                .map_or(tokens.len(), |(_, next)| first_at(*next));
            first_at(*below).min(end)..end
        })
        .collect()
}

#[cfg(test)]
mod tests;
