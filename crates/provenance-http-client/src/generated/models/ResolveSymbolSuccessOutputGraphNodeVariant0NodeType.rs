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
pub enum ResolveSymbolSuccessOutputGraphNodeVariant0NodeType {
    #[serde(rename = "source")]
    Source,
}
impl ::std::fmt::Display for ResolveSymbolSuccessOutputGraphNodeVariant0NodeType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Source => f.write_str("source"),
        }
    }
}
impl ::std::str::FromStr for ResolveSymbolSuccessOutputGraphNodeVariant0NodeType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "source" => Ok(Self::Source),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str>
for ResolveSymbolSuccessOutputGraphNodeVariant0NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for ResolveSymbolSuccessOutputGraphNodeVariant0NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for ResolveSymbolSuccessOutputGraphNodeVariant0NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
