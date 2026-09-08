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
pub enum NeighborsSuccessOutputResolutionMethod {
    #[serde(rename = "grill")]
    Grill,
    #[serde(rename = "prototype")]
    Prototype,
    #[serde(rename = "research")]
    Research,
    #[serde(rename = "verify")]
    Verify,
    #[serde(rename = "task")]
    Task,
}
impl ::std::fmt::Display for NeighborsSuccessOutputResolutionMethod {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Grill => f.write_str("grill"),
            Self::Prototype => f.write_str("prototype"),
            Self::Research => f.write_str("research"),
            Self::Verify => f.write_str("verify"),
            Self::Task => f.write_str("task"),
        }
    }
}
impl ::std::str::FromStr for NeighborsSuccessOutputResolutionMethod {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "grill" => Ok(Self::Grill),
            "prototype" => Ok(Self::Prototype),
            "research" => Ok(Self::Research),
            "verify" => Ok(Self::Verify),
            "task" => Ok(Self::Task),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for NeighborsSuccessOutputResolutionMethod {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for NeighborsSuccessOutputResolutionMethod {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for NeighborsSuccessOutputResolutionMethod {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
