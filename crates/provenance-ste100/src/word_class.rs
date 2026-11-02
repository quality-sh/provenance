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

/// Articles and determiners. A noun can follow each of them, and a verb can
/// take a noun phrase that starts with one of them.
const DETERMINERS: &[&str] = &[
    "a", "an", "the", "this", "these", "those", "each", "every", "any", "some", "no", "all", "my",
    "your", "his", "its", "our", "their",
];

/// Prepositions. A noun can follow each of them.
const PREPOSITIONS: &[&str] = &[
    "of", "in", "on", "at", "for", "with", "from", "by", "into", "onto", "through", "under",
    "over", "between", "without", "about",
];

/// Modal verbs and subject pronouns.
const VERB_CUES: &[&str] = &[
    "can", "could", "may", "might", "must", "shall", "should", "will", "would", "i", "you", "he",
    "she", "it", "we", "they",
];

/// Forms of "be" and "have". A participle can follow each of them.
const AUXILIARY_CUES: &[&str] = &[
    "am", "is", "are", "was", "were", "be", "been", "being", "has", "have", "had",
];

/// Guesses the word class of the token at `position` from the words next to it.
///
/// A token at the start of a sentence is an imperative verb when an article
/// or a determiner follows it, except for a form that ends in "s". After
/// "to", the token is a verb only when an article or a determiner follows
/// it, because "to" is also a preposition. After a form of "be" or "have",
/// only a form that ends in "ed" or "ing" is a verb, because a noun or an
/// adjective can also follow those words. The guess is `None` when no cue
/// settles it.
#[rule("rule_ste_dictionary_sentence_word_class")]
pub fn guess(
    text: &str,
    tokens: &[Token],
    position: usize,
    sentences: &[Sentence],
) -> Option<WordClass> {
    let token = &tokens[position];
    let sentence = sentences
        .iter()
        .find(|sentence| (sentence.start..sentence.end).contains(&token.start));
    let prior = position.checked_sub(1).map(|prior| &tokens[prior]);
    let first_in_sentence =
        prior.is_none_or(|prior| sentence.is_some_and(|sentence| prior.start < sentence.start));
    let determiner_follows = tokens.get(position + 1).is_some_and(|next| {
        text[token.end..next.start].chars().all(char::is_whitespace)
            && DETERMINERS.contains(&next.folded.as_str())
    });

    if first_in_sentence {
        let determinate = sentence.is_some_and(|sentence| !sentence.indeterminate);
        let ends_in_s = token.folded.ends_with('s');
        return (determinate && !ends_in_s && determiner_follows).then_some(WordClass::Verb);
    }
    let prior = prior?;
    if !text[prior.end..token.start]
        .chars()
        .all(char::is_whitespace)
    {
        return None;
    }
    let cue = prior.folded.as_str();
    let participle = token.folded.ends_with("ed") || token.folded.ends_with("ing");
    if cue.ends_with("\u{27}s")
        || cue.ends_with("\u{2019}s")
        || DETERMINERS.contains(&cue)
        || PREPOSITIONS.contains(&cue)
    {
        Some(WordClass::Noun)
    } else if VERB_CUES.contains(&cue)
        || (cue == "to" && determiner_follows)
        || (AUXILIARY_CUES.contains(&cue) && participle)
    {
        Some(WordClass::Verb)
    } else {
        None
    }
}
