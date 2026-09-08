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
pub enum TraceFailureOutputInvalidInputReason {
    #[serde(rename = "required")]
    Required,
    #[serde(rename = "invalid_value")]
    InvalidValue,
    #[serde(rename = "malformed_json")]
    MalformedJson,
    #[serde(rename = "unknown_field")]
    UnknownField,
    #[serde(rename = "too_large")]
    TooLarge,
}
impl ::std::fmt::Display for TraceFailureOutputInvalidInputReason {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Required => f.write_str("required"),
            Self::InvalidValue => f.write_str("invalid_value"),
            Self::MalformedJson => f.write_str("malformed_json"),
            Self::UnknownField => f.write_str("unknown_field"),
            Self::TooLarge => f.write_str("too_large"),
        }
    }
}
impl ::std::str::FromStr for TraceFailureOutputInvalidInputReason {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "required" => Ok(Self::Required),
            "invalid_value" => Ok(Self::InvalidValue),
            "malformed_json" => Ok(Self::MalformedJson),
            "unknown_field" => Ok(Self::UnknownField),
            "too_large" => Ok(Self::TooLarge),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TraceFailureOutputInvalidInputReason {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for TraceFailureOutputInvalidInputReason {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for TraceFailureOutputInvalidInputReason {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
