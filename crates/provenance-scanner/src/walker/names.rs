//! The item names of the sites of one file, given after the walk, when the
//! line of each site is known.
//!
//! The read of a comment site stops at the next comment site, and the read of
//! an attribute or a decorator stops at the next site. An attribute or a
//! decorator directly above the next site, as in a stack of attributes, takes
//! the name of that site. So the reads of the comment sites of a file do not
//! overlap, the reads of the other sites do not overlap, and a bracket that
//! does not close costs one read of the file and not one read for each site.

use std::ops::Range;

use provenance_core::coverage::EvidenceAnchor;

use super::declaration::{self, LineStart};
use super::{script_call_name, AnnotationLocation, AttributeBinding, Language};

/// The sites of one file and the walker's lexical state below each site that
/// the reader names.
#[derive(Default)]
pub(super) struct Sites<'a> {
    comment_lines: Vec<usize>,
    site_lines: Vec<usize>,
    unnamed: Vec<Unnamed<'a>>,
}

struct Unnamed<'a> {
    line: usize,
    /// The index of the first line below the site.
    below: usize,
    start: LineStart<'a>,
    site: Site,
}

enum Site {
    /// The annotations of one comment, at these indexes.
    Comment(Range<usize>),
    /// The binding at this index.
    Binding(usize),
}

impl<'a> Sites<'a> {
    /// A comment on line `line` that holds the annotations at `annotations`.
    pub(super) fn comment(&mut self, line: usize, start: LineStart<'a>, annotations: Range<usize>) {
        self.comment_lines.push(line);
        self.site_lines.push(line);
        self.unnamed.push(Unnamed {
            line,
            below: line + 1,
            start,
            site: Site::Comment(annotations),
        });
    }

    /// An attribute, a decorator, or a call from `line` to `last`, at the
    /// index `binding` of the bindings of the file.
    pub(super) fn binding(
        &mut self,
        line: usize,
        last: usize,
        start: LineStart<'a>,
        binding: usize,
    ) {
        self.site_lines.push(line);
        self.unnamed.push(Unnamed {
            line,
            below: last + 1,
            start,
            site: Site::Binding(binding),
        });
    }

    /// Gives each site its item name and the anchor of that name. A
    /// JavaScript or TypeScript call takes its name by the rules of
    /// `script_call_name`.
    pub(super) fn name(
        self,
        language: Language,
        lines: &[&str],
        original_lines: &[&str],
        annotations: &mut [AnnotationLocation],
        bindings: &mut [AttributeBinding],
    ) {
        let mut next_site: Option<(usize, Option<String>)> = None;
        for unnamed in self.unnamed.into_iter().rev() {
            let boundaries = match unnamed.site {
                Site::Comment(_) => &self.comment_lines,
                Site::Binding(_) => &self.site_lines,
            };
            let next = boundaries.partition_point(|line| *line < unnamed.below);
            let end = boundaries.get(next).map_or(lines.len(), |line| *line);
            let below = &lines[unnamed.below.min(end)..end];
            let binding = match unnamed.site {
                Site::Binding(index) => Some(index),
                Site::Comment(_) => None,
            };
            let stacked = next_site
                .take()
                .filter(|(line, _)| binding.is_some() && *line == unnamed.below);
            let name = match (binding, stacked) {
                (Some(index), _) if !matches!(language, Language::Rust | Language::Python) => {
                    script_call_name(lines, unnamed.below - 1, bindings[index].verification)
                }
                (_, Some((_, name))) => name,
                _ => declaration::item_name(language, below, unnamed.start),
            };
            next_site = Some((unnamed.line, name.clone()));
            match unnamed.site {
                Site::Comment(range) => {
                    for annotation in &mut annotations[range] {
                        let line = original_lines[annotation.line - 1];
                        annotation.anchor = EvidenceAnchor::new(name.clone(), line);
                        annotation.function_name.clone_from(&name);
                    }
                }
                Site::Binding(index) => {
                    let binding = &mut bindings[index];
                    let line = original_lines[binding.line - 1];
                    binding.anchor = EvidenceAnchor::new(name.clone(), line);
                    binding.item_name = name;
                }
            }
        }
    }
}
