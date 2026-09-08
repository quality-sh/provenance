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
pub enum PlanFailureOutputTypedResourceKind {
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "requirement")]
    Requirement,
    #[serde(rename = "rule")]
    Rule,
}
impl ::std::fmt::Display for PlanFailureOutputTypedResourceKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Source => f.write_str("source"),
            Self::Requirement => f.write_str("requirement"),
            Self::Rule => f.write_str("rule"),
        }
    }
}
impl ::std::str::FromStr for PlanFailureOutputTypedResourceKind {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "source" => Ok(Self::Source),
            "requirement" => Ok(Self::Requirement),
            "rule" => Ok(Self::Rule),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlanFailureOutputTypedResourceKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for PlanFailureOutputTypedResourceKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for PlanFailureOutputTypedResourceKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
