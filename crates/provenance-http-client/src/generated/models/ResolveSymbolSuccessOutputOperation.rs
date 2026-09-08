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
pub enum ResolveSymbolSuccessOutputOperation {
    #[serde(rename = "resolve-symbol")]
    ResolveSymbol,
}
impl ::std::fmt::Display for ResolveSymbolSuccessOutputOperation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ResolveSymbol => f.write_str("resolve-symbol"),
        }
    }
}
impl ::std::str::FromStr for ResolveSymbolSuccessOutputOperation {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "resolve-symbol" => Ok(Self::ResolveSymbol),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ResolveSymbolSuccessOutputOperation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for ResolveSymbolSuccessOutputOperation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for ResolveSymbolSuccessOutputOperation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
