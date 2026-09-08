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
pub enum ResolveSymbolSuccessOutputGraphNodeVariant1NodeType {
    #[serde(rename = "requirement")]
    Requirement,
}
impl ::std::fmt::Display for ResolveSymbolSuccessOutputGraphNodeVariant1NodeType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Requirement => f.write_str("requirement"),
        }
    }
}
impl ::std::str::FromStr for ResolveSymbolSuccessOutputGraphNodeVariant1NodeType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "requirement" => Ok(Self::Requirement),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str>
for ResolveSymbolSuccessOutputGraphNodeVariant1NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for ResolveSymbolSuccessOutputGraphNodeVariant1NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for ResolveSymbolSuccessOutputGraphNodeVariant1NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
