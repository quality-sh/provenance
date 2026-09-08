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
pub enum CreateRuleSuccessOutputRuleStatus {
    #[serde(rename = "draft")]
    Draft,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "deprecated")]
    Deprecated,
    #[serde(rename = "archived")]
    Archived,
}
impl ::std::fmt::Display for CreateRuleSuccessOutputRuleStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Draft => f.write_str("draft"),
            Self::Review => f.write_str("review"),
            Self::Active => f.write_str("active"),
            Self::Deprecated => f.write_str("deprecated"),
            Self::Archived => f.write_str("archived"),
        }
    }
}
impl ::std::str::FromStr for CreateRuleSuccessOutputRuleStatus {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "draft" => Ok(Self::Draft),
            "review" => Ok(Self::Review),
            "active" => Ok(Self::Active),
            "deprecated" => Ok(Self::Deprecated),
            "archived" => Ok(Self::Archived),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CreateRuleSuccessOutputRuleStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for CreateRuleSuccessOutputRuleStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for CreateRuleSuccessOutputRuleStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
