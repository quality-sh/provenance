//! Deterministic descriptive-text checks from ASD-STE100 Issue 9.

mod contracted_verbs;
mod dictionary;
mod paragraph;
mod protected_spans;
mod vocabulary;

pub use dictionary::{
    import_dictionary, load_dictionary_index, store_dictionary_index, DictionaryEntry,
    DictionaryImport, DictionaryImportError, DictionaryImportIdentity, DictionaryIndexError,
    DictionaryStatus, PartOfSpeech,
};

use provenance_macros::rule;
use serde::{Deserialize, Serialize};

pub(crate) mod sentence;
pub(crate) mod word_count;

/// The analyzer implementation version included in every report.
pub const ANALYZER_VERSION: &str = env!("CARGO_PKG_VERSION");

const SEMICOLON_MESSAGE: &str = "Do not use semicolons in descriptive text.";
const CONTRACTED_VERB_MESSAGE: &str = "Use the full verb form in descriptive text.";
const SENTENCE_LENGTH_MESSAGE: &str = "This descriptive sentence has more than 25 words.";
const PARAGRAPH_LENGTH_MESSAGE: &str = "This paragraph has more than six sentences.";
const UNAPPROVED_WORD_MESSAGE: &str = "Do not use unapproved dictionary words in descriptive text.";

/// A report from the fixed ASD-STE100 Issue 9 analyzer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub standard: Standard,
    pub issue: StandardIssue,
    pub analyzer_version: String,
    pub findings: Vec<Finding>,
}

/// The authority used by the analyzer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Standard {
    #[serde(rename = "ASD-STE100")]
    AsdSte100,
}

/// The fixed issue of the standard used by the analyzer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
#[cfg_attr(feature = "schema", schemars(extend("const" = 9)))]
pub enum StandardIssue {
    Nine,
}

impl From<StandardIssue> for u8 {
    fn from(issue: StandardIssue) -> Self {
        match issue {
            StandardIssue::Nine => 9,
        }
    }
}

impl TryFrom<u8> for StandardIssue {
    type Error = &'static str;

    fn try_from(issue: u8) -> Result<Self, Self::Error> {
        match issue {
            9 => Ok(Self::Nine),
            _ => Err("the analyzer supports only ASD-STE100 Issue 9"),
        }
    }
}

/// One nonconformance found in descriptive text.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub rule: RuleNumber,
    pub kind: FindingKind,
    pub span: Span,
    pub message: String,
}

/// An ASD-STE100 Issue 9 rule implemented by this analyzer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RuleNumber {
    #[serde(rename = "1.1")]
    OneOne,
    #[serde(rename = "4.2")]
    FourTwo,
    #[serde(rename = "6.3")]
    SixThree,
    #[serde(rename = "6.6")]
    SixSix,
    #[serde(rename = "8.1")]
    EightOne,
}

/// The disposition of a finding.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    Violation,
}

/// A half-open UTF-8 byte range in the analyzed text.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// One word or phrase classified against the imported dictionary.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WordUse {
    pub span: Span,
    pub category: VocabularyCategory,
}

/// Dictionary membership for one word or phrase.
///
/// Membership cannot settle a restricted meaning or part of speech, so an
/// approved word can still break Rule 1.2 or Rule 1.3.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VocabularyCategory {
    Approved,
    Unapproved,
    Unknown,
    Uncertain,
}

/// Reports implemented, determinate ASD-STE100 violations in descriptive text.
#[rule("rule_ste100_semicolon")]
pub fn check_descriptive(text: &str) -> Report {
    let protected = protected_spans::analyze(text);
    let mut findings: Vec<_> = text
        .match_indices(';')
        .filter(|(start, _)| !protected.protects(*start, *start + 1))
        .map(|(start, _)| Finding {
            rule: RuleNumber::EightOne,
            kind: FindingKind::Violation,
            span: Span {
                start,
                end: start + 1,
            },
            message: SEMICOLON_MESSAGE.to_owned(),
        })
        .collect::<Vec<_>>();

    findings.extend(
        contracted_verbs::find(text, &protected)
            .into_iter()
            .map(|contracted| Finding {
                rule: RuleNumber::FourTwo,
                kind: FindingKind::Violation,
                span: Span {
                    start: contracted.start,
                    end: contracted.end,
                },
                message: CONTRACTED_VERB_MESSAGE.to_owned(),
            }),
    );
    findings.extend(sentence_length_findings(text, &protected));
    findings.extend(paragraph_sentence_findings(text, &protected));
    sort_findings(&mut findings);

    Report {
        standard: Standard::AsdSte100,
        issue: StandardIssue::Nine,
        analyzer_version: ANALYZER_VERSION.to_owned(),
        findings,
    }
}

/// Reports determinate violations, with Rule 1.1 vocabulary findings from the
/// imported dictionary added to the data-free checks.
pub fn check_descriptive_with_dictionary(text: &str, dictionary: &DictionaryImport) -> Report {
    let mut report = check_descriptive(text);
    let protected = protected_spans::analyze(text);
    report
        .findings
        .extend(unapproved_word_findings(text, &protected, dictionary));
    sort_findings(&mut report.findings);
    report
}

/// Reports each word whose imported dictionary forms are all unapproved.
#[rule("rule_ste_dictionary_unapproved_word")]
fn unapproved_word_findings(
    text: &str,
    protected: &protected_spans::ProtectedSpans,
    dictionary: &DictionaryImport,
) -> Vec<Finding> {
    vocabulary::classify(text, protected, dictionary)
        .into_iter()
        .filter(|word_use| word_use.category == VocabularyCategory::Unapproved)
        .map(|word_use| Finding {
            rule: RuleNumber::OneOne,
            kind: FindingKind::Violation,
            span: word_use.span,
            message: UNAPPROVED_WORD_MESSAGE.to_owned(),
        })
        .collect()
}

fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by_key(|finding| {
        (
            finding.span.start,
            finding.span.end,
            match finding.rule {
                RuleNumber::OneOne => 0,
                RuleNumber::FourTwo => 1,
                RuleNumber::SixThree => 2,
                RuleNumber::SixSix => 3,
                RuleNumber::EightOne => 4,
            },
        )
    });
}

/// Classifies every word outside protected text against the imported dictionary.
pub fn classify_vocabulary(text: &str, dictionary: &DictionaryImport) -> Vec<WordUse> {
    let protected = protected_spans::analyze(text);
    vocabulary::classify(text, &protected, dictionary)
}

/// Reports each paragraph with clear boundaries that has more than six sentences.
#[rule("rule_ste100_paragraph_sentence_limit")]
fn paragraph_sentence_findings(
    text: &str,
    protected: &protected_spans::ProtectedSpans,
) -> Vec<Finding> {
    let Some(paragraphs) = paragraph::scan(text, protected) else {
        return Vec::new();
    };

    paragraphs
        .into_iter()
        .filter_map(|paragraph| {
            let sentences = sentence::scan_range(text, protected, paragraph.clone());
            (sentences.len() > 6 && sentences.iter().all(|sentence| !sentence.indeterminate)).then(
                || Finding {
                    rule: RuleNumber::SixSix,
                    kind: FindingKind::Violation,
                    span: Span {
                        start: paragraph.start,
                        end: paragraph.end,
                    },
                    message: PARAGRAPH_LENGTH_MESSAGE.to_owned(),
                },
            )
        })
        .collect()
}

/// Reports each determinate descriptive sentence that has more than 25 words.
#[rule("rule_ste100_descriptive_sentence_length")]
fn sentence_length_findings(
    text: &str,
    protected: &protected_spans::ProtectedSpans,
) -> Vec<Finding> {
    sentence::scan(text, protected)
        .into_iter()
        .filter_map(
            |sentence| match word_count::count(text, sentence, protected) {
                word_count::Count::Exact(count) if count > 25 => Some(Finding {
                    rule: RuleNumber::SixThree,
                    kind: FindingKind::Violation,
                    span: Span {
                        start: sentence.start,
                        end: sentence.end,
                    },
                    message: SENTENCE_LENGTH_MESSAGE.to_owned(),
                }),
                word_count::Count::Exact(_) | word_count::Count::Indeterminate => None,
            },
        )
        .collect()
}
