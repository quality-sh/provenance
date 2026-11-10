//! The approved alternatives that the imported dictionary gives for an
//! unapproved word.

use crate::dictionary::{DictionaryImport, DictionaryStatus};
use crate::inflection;

/// Gives the text of the Approved meaning or alternatives column for each
/// unapproved entry of the rejected word, in dictionary order. A printed form
/// wins. Otherwise the base forms of a regular ending are used.
pub fn for_rejected(dictionary: &DictionaryImport, rejected: &str) -> Vec<String> {
    let folded = rejected
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let printed = matching(dictionary, &[folded.as_str()]);
    if !printed.is_empty() || folded.contains(' ') {
        return printed;
    }
    let bases: Vec<String> = inflection::base_candidates(&folded)
        .into_iter()
        .map(|(base, _)| base)
        .collect();
    let bases: Vec<&str> = std::iter::once(folded.strip_suffix("\u{27}s").unwrap_or(&folded))
        .chain(bases.iter().map(String::as_str))
        .collect();
    matching(dictionary, &bases)
}

fn matching(dictionary: &DictionaryImport, candidates: &[&str]) -> Vec<String> {
    let mut alternatives: Vec<String> = Vec::new();
    for entry in &dictionary.entries {
        let unapproved = entry.status == DictionaryStatus::Unapproved;
        let matches = entry.word_forms.iter().any(|form| {
            let form = form.split_whitespace().collect::<Vec<_>>().join(" ");
            candidates.contains(&form.to_lowercase().as_str())
        });
        let text = entry
            .approved_meaning_or_alternatives
            .trim()
            .trim_end_matches('.');
        if unapproved
            && matches
            && !text.is_empty()
            && !alternatives.iter().any(|seen| seen == text)
        {
            alternatives.push(text.to_owned());
        }
    }
    alternatives
}
