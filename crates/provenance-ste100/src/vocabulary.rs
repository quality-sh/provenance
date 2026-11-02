use std::collections::HashMap;

use provenance_macros::rule;

use crate::inflection::{self, Inflection};
use crate::protected_spans::ProtectedSpans;
use crate::sentence;
use crate::word_class;
use crate::{DictionaryImport, DictionaryStatus, PartOfSpeech, Span, VocabularyCategory, WordUse};

/// Classifies each word or phrase by dictionary membership only.
#[rule("rule_ste_dictionary_finding_categories")]
pub fn classify(
    text: &str,
    protected: &ProtectedSpans,
    dictionary: &DictionaryImport,
) -> Vec<WordUse> {
    let index = VocabularyIndex::build(dictionary);
    let tokens = scan_tokens(text);
    let sentences = sentence::scan(text, protected);
    let mut uses = Vec::new();
    let mut position = 0;

    while position < tokens.len() {
        let token = &tokens[position];
        if token.identifier || protected.protects(token.start, token.end) {
            position += 1;
            continue;
        }
        if let Some(matched) = index.longest_phrase(text, protected, &tokens, position) {
            uses.push(WordUse {
                span: Span {
                    start: token.start,
                    end: tokens[position + matched.length - 1].end,
                },
                category: matched.category,
            });
            position += matched.length;
            continue;
        }
        uses.push(WordUse {
            span: Span {
                start: token.start,
                end: token.end,
            },
            category: settle(&index.word_matches(&token.folded), || {
                word_class::guess(text, &tokens, position, &sentences)
            }),
        });
        position += 1;
    }

    uses
}

/// Finds each listed technical name in the text, independent of the
/// dictionary phrases. A name matches with the dictionary folding: case, the
/// spaces between words, and a possessive "'s" on the last word.
#[rule("rule_ste_technical_names_accepted")]
pub fn technical_name_spans(text: &str, technical_names: &[String]) -> Vec<Span> {
    let names: Vec<Vec<String>> = technical_names
        .iter()
        .map(|name| {
            name.to_lowercase()
                .split_whitespace()
                .map(str::to_owned)
                .collect()
        })
        .filter(|words: &Vec<String>| !words.is_empty())
        .collect();
    let tokens = scan_tokens(text);
    let mut spans = Vec::new();
    for (position, token) in tokens.iter().enumerate() {
        for words in &names {
            if name_matches(text, &tokens, position, words) {
                spans.push(Span {
                    start: token.start,
                    end: tokens[position + words.len() - 1].end,
                });
            }
        }
    }
    spans
}

fn name_matches(text: &str, tokens: &[Token], position: usize, words: &[String]) -> bool {
    if position + words.len() > tokens.len() {
        return false;
    }
    words.iter().enumerate().all(|(offset, word)| {
        let token = &tokens[position + offset];
        let last = offset + 1 == words.len();
        let folded_matches = token.folded == *word
            || (last && token.folded.strip_suffix("\u{27}s") == Some(word.as_str()));
        let gap_is_space = offset == 0
            || text[tokens[position + offset - 1].end..token.start]
                .chars()
                .all(char::is_whitespace);
        folded_matches && gap_is_space
    })
}

pub struct Token {
    pub start: usize,
    pub end: usize,
    pub folded: String,
    identifier: bool,
}

fn scan_tokens(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let characters: Vec<(usize, char)> = text.char_indices().collect();
    let mut position = 0;

    while position < characters.len() {
        let (start, character) = characters[position];
        if !character.is_alphabetic() {
            position += 1;
            continue;
        }
        let mut close = position;
        while close + 1 < characters.len() && continues_word(&characters, close + 1) {
            close += 1;
        }
        let end = characters[close].0 + characters[close].1.len_utf8();
        let before = characters[..position].last().map(|(_, prior)| *prior);
        let after = characters.get(close + 1).map(|(_, next)| *next);
        tokens.push(Token {
            start,
            end,
            folded: text[start..end].to_lowercase(),
            identifier: is_identifier_edge(before) || is_identifier_edge(after),
        });
        position = close + 1;
    }

    tokens
}

fn continues_word(characters: &[(usize, char)], position: usize) -> bool {
    let (_, character) = characters[position];
    if character.is_alphabetic() {
        return true;
    }
    if character != '\u{27}' && character != '\u{2019}' {
        return false;
    }
    characters
        .get(position + 1)
        .is_some_and(|(_, next)| next.is_alphabetic())
}

fn is_identifier_edge(character: Option<char>) -> bool {
    character.is_some_and(|character| character == '_' || character.is_ascii_digit())
}

struct PhraseMatch {
    length: usize,
    category: VocabularyCategory,
}

/// One dictionary entry that a token can stand for.
type EntryMatch = (PartOfSpeech, DictionaryStatus);

struct VocabularyIndex {
    words: HashMap<String, Vec<EntryMatch>>,
    phrases: HashMap<String, Vec<(Vec<String>, DictionaryStatus)>>,
}

impl VocabularyIndex {
    fn build(dictionary: &DictionaryImport) -> Self {
        let mut index = Self {
            words: HashMap::new(),
            phrases: HashMap::new(),
        };
        for entry in &dictionary.entries {
            for form in &entry.word_forms {
                let folded = form.to_lowercase();
                let words: Vec<String> = folded.split_whitespace().map(str::to_owned).collect();
                match words.as_slice() {
                    [] => {}
                    [word] => {
                        let matches = index.words.entry(word.clone()).or_default();
                        let found = (entry.part_of_speech, entry.status);
                        if !matches.contains(&found) {
                            matches.push(found);
                        }
                    }
                    [first, ..] => {
                        index
                            .phrases
                            .entry(first.clone())
                            .or_default()
                            .push((words, entry.status));
                    }
                }
            }
        }
        index
    }

    /// Gives the entries that a token can stand for. A printed form wins.
    /// Otherwise a regular ending is removed: a verb ending matches only an
    /// unapproved verb, and a plural matches a noun of either status.
    fn word_matches(&self, folded: &str) -> Vec<EntryMatch> {
        let base = folded.strip_suffix("\u{27}s").unwrap_or(folded);
        let printed: Vec<EntryMatch> = [folded, base]
            .iter()
            .filter_map(|candidate| self.words.get(*candidate))
            .flatten()
            .copied()
            .collect();
        if !printed.is_empty() {
            return printed;
        }
        inflection::base_candidates(folded)
            .into_iter()
            .filter_map(|(candidate, ending)| {
                self.words.get(&candidate).map(|matches| (matches, ending))
            })
            .flat_map(|(matches, ending)| {
                matches
                    .iter()
                    .copied()
                    .filter(move |found| derives(ending, *found))
            })
            .collect()
    }

    fn longest_phrase(
        &self,
        text: &str,
        protected: &ProtectedSpans,
        tokens: &[Token],
        position: usize,
    ) -> Option<PhraseMatch> {
        let candidates = self.phrases.get(&tokens[position].folded)?;
        let matching: Vec<&(Vec<String>, DictionaryStatus)> = candidates
            .iter()
            .filter(|(words, _)| phrase_matches(text, protected, tokens, position, words))
            .collect();
        let length = matching.iter().map(|(words, _)| words.len()).max()?;
        let status_present = |wanted: DictionaryStatus| {
            matching
                .iter()
                .any(|(words, status)| words.len() == length && *status == wanted)
        };
        Some(PhraseMatch {
            length,
            category: category_of(
                status_present(DictionaryStatus::Approved),
                status_present(DictionaryStatus::Unapproved),
            ),
        })
    }
}

fn phrase_matches(
    text: &str,
    protected: &ProtectedSpans,
    tokens: &[Token],
    position: usize,
    words: &[String],
) -> bool {
    if position + words.len() > tokens.len() {
        return false;
    }
    for (offset, word) in words.iter().enumerate() {
        let token = &tokens[position + offset];
        if token.identifier || protected.protects(token.start, token.end) || token.folded != *word {
            return false;
        }
        if offset > 0 {
            let gap = &text[tokens[position + offset - 1].end..token.start];
            if !gap.chars().all(char::is_whitespace) {
                return false;
            }
        }
    }
    true
}

fn derives(ending: Inflection, (part_of_speech, status): EntryMatch) -> bool {
    part_of_speech == ending.part_of_speech()
        && (ending == Inflection::Plural || status == DictionaryStatus::Unapproved)
}

/// Gives the category of a word. When the word has approved and unapproved
/// entries, the word class from the sentence picks the entries. A word class
/// with no entry, or no word class, leaves the word uncertain.
fn settle(
    matches: &[EntryMatch],
    guess: impl FnOnce() -> Option<word_class::WordClass>,
) -> VocabularyCategory {
    let category = status_category(matches);
    if category != VocabularyCategory::Uncertain {
        return category;
    }
    let Some(class) = guess() else {
        return category;
    };
    let picked: Vec<EntryMatch> = matches
        .iter()
        .copied()
        .filter(|(part_of_speech, _)| *part_of_speech == class.part_of_speech())
        .collect();
    if picked.is_empty() {
        category
    } else {
        status_category(&picked)
    }
}

fn status_category(matches: &[EntryMatch]) -> VocabularyCategory {
    category_of(
        matches
            .iter()
            .any(|(_, status)| *status == DictionaryStatus::Approved),
        matches
            .iter()
            .any(|(_, status)| *status == DictionaryStatus::Unapproved),
    )
}

const fn category_of(approved: bool, unapproved: bool) -> VocabularyCategory {
    match (approved, unapproved) {
        (true, true) => VocabularyCategory::Uncertain,
        (true, false) => VocabularyCategory::Approved,
        (false, true) => VocabularyCategory::Unapproved,
        (false, false) => VocabularyCategory::Unknown,
    }
}
