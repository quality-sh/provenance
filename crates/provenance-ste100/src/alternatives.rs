//! The dictionary text that a Rule 1.1 finding gives for a rejected word.

use crate::dictionary::DictionaryImport;
use provenance_macros::rule;

/// Gives the sentence that quotes the "approved meaning or alternatives" text
/// of the entries that rejected a word, in dictionary order.
///
/// The column can hold an approved meaning, an alternative, or a note, so the
/// sentence quotes it and does not call it an alternative. An entry with no
/// text adds nothing, and the same text occurs one time.
#[rule("rule_ste_word_refusal_gives_dictionary_text")]
pub fn given(dictionary: &DictionaryImport, entries: &[usize]) -> String {
    let mut ordered = entries.to_vec();
    ordered.sort_unstable();
    ordered.dedup();
    let mut texts: Vec<&str> = Vec::new();
    for entry in ordered
        .iter()
        .filter_map(|index| dictionary.entries.get(*index))
    {
        let text = entry
            .approved_meaning_or_alternatives
            .trim()
            .trim_end_matches('.');
        if !text.is_empty() && !texts.contains(&text) {
            texts.push(text);
        }
    }
    if texts.is_empty() {
        String::new()
    } else {
        format!(" The dictionary gives: {}.", texts.join("; "))
    }
}

#[cfg(test)]
mod tests;
