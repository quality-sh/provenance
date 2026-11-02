//! A word-class guess from the words next to a token.

use provenance_macros::rule;

use crate::sentence::Sentence;
use crate::vocabulary::Token;
use crate::PartOfSpeech;

/// The word class that a sentence can give a dictionary word.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WordClass {
    Noun,
    Verb,
}

impl WordClass {
    pub const fn part_of_speech(self) -> PartOfSpeech {
        match self {
            Self::Noun => PartOfSpeech::Noun,
            Self::Verb => PartOfSpeech::Verb,
        }
    }
}

/// Articles, determiners and prepositions. A noun can follow each of them.
const NOUN_CUES: &[&str] = &[
    "a", "an", "the", "this", "these", "those", "each", "every", "any", "some", "no", "all", "my",
    "your", "his", "its", "our", "their", "of", "in", "on", "at", "for", "with", "from", "by",
    "into", "onto", "through", "under", "over", "between", "without", "about",
];

/// The infinitive marker, modal verbs and subject pronouns.
const VERB_CUES: &[&str] = &[
    "to", "can", "could", "may", "might", "must", "shall", "should", "will", "would", "i", "you",
    "he", "she", "it", "we", "they",
];

/// Forms of "be" and "have". A participle can follow each of them.
const AUXILIARY_CUES: &[&str] = &[
    "am", "is", "are", "was", "were", "be", "been", "being", "has", "have", "had",
];

/// Guesses the word class of the token at `position` from the word before it.
///
/// A token at the start of a sentence is an imperative verb, except for a
/// form that ends in "s". The guess is `None` when no cue settles it.
#[rule("rule_ste_dictionary_sentence_word_class")]
pub fn guess(
    text: &str,
    tokens: &[Token],
    position: usize,
    sentences: &[Sentence],
) -> Option<WordClass> {
    let token = &tokens[position];
    let ends_in_s = token.folded.ends_with('s');
    let sentence = sentences
        .iter()
        .find(|sentence| sentence.start <= token.start && token.start < sentence.end);
    let prior = position.checked_sub(1).map(|prior| &tokens[prior]);
    let first_in_sentence =
        prior.is_none_or(|prior| sentence.is_some_and(|sentence| prior.start < sentence.start));

    if first_in_sentence {
        let determinate = sentence.is_some_and(|sentence| !sentence.indeterminate);
        return (determinate && !ends_in_s).then_some(WordClass::Verb);
    }
    let prior = prior?;
    if !text[prior.end..token.start]
        .chars()
        .all(char::is_whitespace)
    {
        return None;
    }
    let cue = prior.folded.as_str();
    if cue.ends_with("\u{27}s") || cue.ends_with("\u{2019}s") || NOUN_CUES.contains(&cue) {
        Some(WordClass::Noun)
    } else if VERB_CUES.contains(&cue) || (AUXILIARY_CUES.contains(&cue) && !ends_in_s) {
        Some(WordClass::Verb)
    } else {
        None
    }
}
