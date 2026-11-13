//! The range of the integers that a record keeps: each integer that a
//! 64-bit float holds exactly and reads back as the same integer.
//!
//! A 64-bit float holds every integer of magnitude up to 2^53, but it
//! rounds 2^53 + 1 to 2^53. So a float of 2^53 does not tell which of the
//! two integers it came from. Up to 2^53 − 1, each float of an integer
//! reads back as that one integer, and an integer past the range gives a
//! float past the range. The range is closed at ±(2^53 − 1) for that
//! reason.

use std::fmt;

/// The largest magnitude of an integer in the range: 2^53 − 1.
pub const LARGEST_EXACT_INTEGER: i64 = (1 << 53) - 1;

/// Whether `value` is in the range.
pub const fn is_exact(value: i64) -> bool {
    -LARGEST_EXACT_INTEGER <= value && value <= LARGEST_EXACT_INTEGER
}

/// The text of the range, `-9007199254740991 to 9007199254740991`. Each
/// message that names the range writes it with this type.
#[derive(Debug, Clone, Copy)]
pub struct RangeText;

impl fmt::Display for RangeText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} to {LARGEST_EXACT_INTEGER}",
            -LARGEST_EXACT_INTEGER
        )
    }
}

/// What the parse of a field in the range expects.
struct InRange;

impl serde::de::Expected for InRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "an integer from {RangeText}")
    }
}

/// Reads a time field and refuses a value past the range, as serde refuses
/// a value of the wrong type.
pub fn time<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    checked(<i64 as serde::Deserialize>::deserialize(deserializer)?)
}

/// Reads an optional time field and refuses a value past the range.
pub fn optional_time<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    <Option<i64> as serde::Deserialize>::deserialize(deserializer)?
        .map(checked)
        .transpose()
}

/// Reads a count, as the version of a Discussion, and refuses a value past
/// the range.
pub fn count<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = <u64 as serde::Deserialize>::deserialize(deserializer)?;
    if i64::try_from(value).is_ok_and(is_exact) {
        Ok(value)
    } else {
        Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Unsigned(value),
            &InRange,
        ))
    }
}

fn checked<E: serde::de::Error>(value: i64) -> Result<i64, E> {
    if is_exact(value) {
        Ok(value)
    } else {
        Err(E::invalid_value(
            serde::de::Unexpected::Signed(value),
            &InRange,
        ))
    }
}
