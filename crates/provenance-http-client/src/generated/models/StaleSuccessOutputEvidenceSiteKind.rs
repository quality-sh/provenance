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
pub enum StaleSuccessOutputEvidenceSiteKind {
    #[serde(rename = "rule_binding")]
    RuleBinding,
    #[serde(rename = "verification")]
    Verification,
    #[serde(rename = "annotation")]
    Annotation,
    #[serde(rename = "source_reference")]
    SourceReference,
}
impl ::std::fmt::Display for StaleSuccessOutputEvidenceSiteKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::RuleBinding => f.write_str("rule_binding"),
            Self::Verification => f.write_str("verification"),
            Self::Annotation => f.write_str("annotation"),
            Self::SourceReference => f.write_str("source_reference"),
        }
    }
}
impl ::std::str::FromStr for StaleSuccessOutputEvidenceSiteKind {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "rule_binding" => Ok(Self::RuleBinding),
            "verification" => Ok(Self::Verification),
            "annotation" => Ok(Self::Annotation),
            "source_reference" => Ok(Self::SourceReference),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for StaleSuccessOutputEvidenceSiteKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for StaleSuccessOutputEvidenceSiteKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for StaleSuccessOutputEvidenceSiteKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
