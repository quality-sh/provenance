mod digest;
mod index;
mod layout;
mod parse;
mod pdf;

pub use index::{
    load_dictionary_index, load_dictionary_index_for_source, store_dictionary_index,
    DictionaryIndexError,
};

use provenance_macros::rule;
use serde::{Deserialize, Serialize};

use crate::StandardIssue;

pub const DICTIONARY_EXTRACTOR_VERSION: &str = env!("CARGO_PKG_VERSION");

const EXPECTED_APPROVED_ROWS: usize = 878;
const EXPECTED_UNAPPROVED_ROWS: usize = 1_318;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DictionaryImport {
    pub identity: DictionaryImportIdentity,
    pub entries: Vec<DictionaryEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DictionaryImportIdentity {
    pub issue: StandardIssue,
    pub source_sha256: String,
    pub data_sha256: String,
    pub extractor_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub headword: String,
    pub word_forms: Vec<String>,
    pub part_of_speech: PartOfSpeech,
    pub status: DictionaryStatus,
    pub approved_meaning_or_alternatives: String,
    pub ste_example: String,
    pub non_ste_example: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryStatus {
    Approved,
    Unapproved,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartOfSpeech {
    Adjective,
    Adverb,
    Article,
    Conjunction,
    Noun,
    Prefix,
    Preposition,
    Pronoun,
    Verb,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DictionaryImportError {
    InvalidPdf {
        message: String,
    },
    UnsupportedDocument {
        reason: String,
    },
    UnreliableText {
        page: usize,
        reason: String,
    },
    DictionaryNotFound,
    InvalidStructure {
        approved_rows: usize,
        unapproved_rows: usize,
        reason: String,
    },
}

impl std::fmt::Display for DictionaryImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPdf { message } => write!(formatter, "invalid PDF: {message}"),
            Self::UnsupportedDocument { reason } => {
                write!(formatter, "unsupported document: {reason}")
            }
            Self::UnreliableText { page, reason } => {
                write!(formatter, "unreliable text on PDF page {page}: {reason}")
            }
            Self::DictionaryNotFound => formatter.write_str("Issue 9 dictionary pages not found"),
            Self::InvalidStructure { reason, .. } => {
                write!(formatter, "invalid Issue 9 dictionary structure: {reason}")
            }
        }
    }
}

impl std::error::Error for DictionaryImportError {}

/// Imports dictionary data only after the PDF identity and table are valid.
pub fn import_dictionary(pdf_bytes: &[u8]) -> Result<DictionaryImport, DictionaryImportError> {
    let pages = pdf::extract_dictionary_pages(pdf_bytes)?;
    let tables = pages
        .iter()
        .map(layout::table_from_page)
        .collect::<Result<Vec<_>, _>>()?;
    let entries = parse::parse_entries(&tables)?;
    validate_entries(&entries)?;

    Ok(DictionaryImport {
        identity: digest::identity(pdf_bytes, &entries),
        entries,
    })
}

/// Rejects incomplete, unordered, or misclassified dictionary data.
#[rule("rule_ste_dictionary_structure_validation")]
fn validate_entries(entries: &[DictionaryEntry]) -> Result<(), DictionaryImportError> {
    let approved = entries
        .iter()
        .filter(|entry| entry.status == DictionaryStatus::Approved)
        .count();
    let unapproved = entries.len() - approved;
    let counts_are_valid =
        approved == EXPECTED_APPROVED_ROWS && unapproved == EXPECTED_UNAPPROVED_ROWS;
    let order_is_valid = entries
        .windows(2)
        .all(|pair| alphabet_band(&pair[0].headword) <= alphabet_band(&pair[1].headword));

    if !counts_are_valid || !order_is_valid {
        let reason = if counts_are_valid {
            "entry alphabet bands are not in dictionary order".to_owned()
        } else {
            format!(
                "expected {EXPECTED_APPROVED_ROWS} approved and {EXPECTED_UNAPPROVED_ROWS} \
                 unapproved table rows; found {approved} and {unapproved}"
            )
        };
        return Err(DictionaryImportError::InvalidStructure {
            approved_rows: approved,
            unapproved_rows: unapproved,
            reason,
        });
    }

    Ok(())
}

fn alphabet_band(headword: &str) -> Option<char> {
    headword
        .chars()
        .find(|character| character.is_alphabetic())
        .map(|character| character.to_ascii_lowercase())
}
