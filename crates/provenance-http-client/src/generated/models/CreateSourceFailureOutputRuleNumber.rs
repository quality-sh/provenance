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
pub enum CreateSourceFailureOutputRuleNumber {
    #[serde(rename = "1.1")]
    X11,
    #[serde(rename = "4.2")]
    X42,
    #[serde(rename = "6.3")]
    X63,
    #[serde(rename = "6.6")]
    X66,
    #[serde(rename = "8.1")]
    X81,
}
impl ::std::fmt::Display for CreateSourceFailureOutputRuleNumber {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::X11 => f.write_str("1.1"),
            Self::X42 => f.write_str("4.2"),
            Self::X63 => f.write_str("6.3"),
            Self::X66 => f.write_str("6.6"),
            Self::X81 => f.write_str("8.1"),
        }
    }
}
impl ::std::str::FromStr for CreateSourceFailureOutputRuleNumber {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "1.1" => Ok(Self::X11),
            "4.2" => Ok(Self::X42),
            "6.3" => Ok(Self::X63),
            "6.6" => Ok(Self::X66),
            "8.1" => Ok(Self::X81),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CreateSourceFailureOutputRuleNumber {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for CreateSourceFailureOutputRuleNumber {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for CreateSourceFailureOutputRuleNumber {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
