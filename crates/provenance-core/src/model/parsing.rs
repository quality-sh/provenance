use serde::de::value::{Error as WordError, StringDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};

pub(super) fn normalize_enum_value(value: &str) -> String {
    value.trim().replace('-', "_").to_ascii_lowercase()
}

/// Reads one word into a closed word-list enum.
///
/// The enum's serde renames are the only word list. The text is normalized
/// first, so `API-Spec` reads as `api_spec`.
pub(super) fn parse_enum_word<T: DeserializeOwned>(value: &str) -> anyhow::Result<T> {
    let word: StringDeserializer<WordError> = normalize_enum_value(value).into_deserializer();
    T::deserialize(word).map_err(anyhow::Error::from)
}
