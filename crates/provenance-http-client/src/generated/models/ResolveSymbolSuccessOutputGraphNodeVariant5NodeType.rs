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
pub enum ResolveSymbolSuccessOutputGraphNodeVariant5NodeType {
    #[serde(rename = "question")]
    Question,
}
impl ::std::fmt::Display for ResolveSymbolSuccessOutputGraphNodeVariant5NodeType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Question => f.write_str("question"),
        }
    }
}
impl ::std::str::FromStr for ResolveSymbolSuccessOutputGraphNodeVariant5NodeType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "question" => Ok(Self::Question),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str>
for ResolveSymbolSuccessOutputGraphNodeVariant5NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for ResolveSymbolSuccessOutputGraphNodeVariant5NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for ResolveSymbolSuccessOutputGraphNodeVariant5NodeType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
