//! Regular English inflections that the dictionary does not print.

use provenance_macros::rule;

use crate::PartOfSpeech;

/// One regular ending that a token can carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Inflection {
    ThirdPerson,
    Past,
    PresentParticiple,
    Plural,
}

impl Inflection {
    /// The part of speech of an entry that can take this ending.
    pub(crate) const fn part_of_speech(self) -> PartOfSpeech {
        match self {
            Self::ThirdPerson | Self::Past | Self::PresentParticiple => PartOfSpeech::Verb,
            Self::Plural => PartOfSpeech::Noun,
        }
    }
}

/// The shortest base, in characters, that an ending can leave.
const MINIMUM_BASE_LENGTH: usize = 2;

/// Gives each base that a regular ending on a folded token can come from.
///
/// The past form also covers the past participle, because regular verbs use
/// one spelling for both. A possessive "'s" is removed first.
#[rule("rule_ste_dictionary_inflected_unapproved_form")]
pub(crate) fn base_candidates(folded: &str) -> Vec<(String, Inflection)> {
    let word = folded.strip_suffix("\u{27}s").unwrap_or(folded);
    let mut candidates = Vec::new();

    if let Some(stem) = word.strip_suffix("ies") {
        push_with_ending(&mut candidates, stem, "y", Inflection::ThirdPerson);
        push_with_ending(&mut candidates, stem, "y", Inflection::Plural);
    }
    if let Some(stem) = word.strip_suffix("es") {
        push(&mut candidates, stem, Inflection::ThirdPerson);
        push(&mut candidates, stem, Inflection::Plural);
    }
    if let Some(stem) = word.strip_suffix('s') {
        if !stem.ends_with('s') {
            push(&mut candidates, stem, Inflection::ThirdPerson);
            push(&mut candidates, stem, Inflection::Plural);
        }
    }
    if let Some(stem) = word.strip_suffix("ied") {
        push_with_ending(&mut candidates, stem, "y", Inflection::Past);
    }
    if let Some(stem) = word.strip_suffix("ed") {
        push_verb_stem(&mut candidates, stem, Inflection::Past);
    }
    if let Some(stem) = word.strip_suffix('d') {
        if stem.ends_with('e') {
            push(&mut candidates, stem, Inflection::Past);
        }
    }
    if let Some(stem) = word.strip_suffix("ing") {
        push_verb_stem(&mut candidates, stem, Inflection::PresentParticiple);
        push_with_ending(&mut candidates, stem, "e", Inflection::PresentParticiple);
    }

    candidates
}

/// Adds the stem, and the stem without a doubled final consonant.
fn push_verb_stem(candidates: &mut Vec<(String, Inflection)>, stem: &str, inflection: Inflection) {
    push(candidates, stem, inflection);
    let mut characters = stem.chars().rev();
    if let (Some(last), Some(prior)) = (characters.next(), characters.next()) {
        if last == prior && is_consonant(last) {
            push(
                candidates,
                &stem[..stem.len() - last.len_utf8()],
                inflection,
            );
        }
    }
}

fn push_with_ending(
    candidates: &mut Vec<(String, Inflection)>,
    stem: &str,
    ending: &str,
    inflection: Inflection,
) {
    push(candidates, &format!("{stem}{ending}"), inflection);
}

fn push(candidates: &mut Vec<(String, Inflection)>, base: &str, inflection: Inflection) {
    if base.chars().count() >= MINIMUM_BASE_LENGTH
        && !candidates
            .iter()
            .any(|(known, known_inflection)| known == base && *known_inflection == inflection)
    {
        candidates.push((base.to_owned(), inflection));
    }
}

fn is_consonant(character: char) -> bool {
    character.is_ascii_alphabetic() && !matches!(character, 'a' | 'e' | 'i' | 'o' | 'u')
}

#[cfg(test)]
mod tests {
    use super::{base_candidates, Inflection};

    fn has(folded: &str, base: &str, inflection: Inflection) -> bool {
        base_candidates(folded)
            .iter()
            .any(|(candidate, found)| candidate == base && *found == inflection)
    }

    #[test]
    fn each_regular_ending_gives_its_base() {
        assert!(has("frobs", "frob", Inflection::ThirdPerson));
        assert!(has("frobs", "frob", Inflection::Plural));
        assert!(has("zaxes", "zax", Inflection::Plural));
        assert!(has("plies", "ply", Inflection::ThirdPerson));
        assert!(has("plied", "ply", Inflection::Past));
        assert!(has("frobbed", "frob", Inflection::Past));
        assert!(has("zorped", "zorp", Inflection::Past));
        assert!(has("glimed", "glime", Inflection::Past));
        assert!(has("zorping", "zorp", Inflection::PresentParticiple));
        assert!(has("gliming", "glime", Inflection::PresentParticiple));
        assert!(has("frobbing", "frob", Inflection::PresentParticiple));
        assert!(base_candidates("zorp's").is_empty());
        assert!(has("zorps's", "zorp", Inflection::Plural));
    }

    #[test]
    fn a_double_s_and_a_short_base_give_no_candidate() {
        assert!(!has("zass", "zas", Inflection::Plural));
        assert!(base_candidates("as").is_empty());
    }
}
