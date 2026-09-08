// Generated from OpenAPI. Do not edit.
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd
)]
pub enum EvidenceSuccessOutputEvidenceDiffState {
    #[serde(rename = "untouched")]
    Untouched,
    #[serde(rename = "touched")]
    Touched,
    #[serde(rename = "moved")]
    Moved,
    #[serde(rename = "gone")]
    Gone,
}
impl ::std::fmt::Display for EvidenceSuccessOutputEvidenceDiffState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Untouched => f.write_str("untouched"),
            Self::Touched => f.write_str("touched"),
            Self::Moved => f.write_str("moved"),
            Self::Gone => f.write_str("gone"),
        }
    }
}
impl ::std::str::FromStr for EvidenceSuccessOutputEvidenceDiffState {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "untouched" => Ok(Self::Untouched),
            "touched" => Ok(Self::Touched),
            "moved" => Ok(Self::Moved),
            "gone" => Ok(Self::Gone),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EvidenceSuccessOutputEvidenceDiffState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for EvidenceSuccessOutputEvidenceDiffState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for EvidenceSuccessOutputEvidenceDiffState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
