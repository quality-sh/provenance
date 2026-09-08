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
pub enum CreateResolutionRequestInputResolutionStatus {
    #[serde(rename = "draft")]
    Draft,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "proposed")]
    Proposed,
    #[serde(rename = "approved")]
    Approved,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "revised")]
    Revised,
    #[serde(rename = "superseded")]
    Superseded,
    #[serde(rename = "abandoned")]
    Abandoned,
}
impl ::std::fmt::Display for CreateResolutionRequestInputResolutionStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Draft => f.write_str("draft"),
            Self::Review => f.write_str("review"),
            Self::Proposed => f.write_str("proposed"),
            Self::Approved => f.write_str("approved"),
            Self::Rejected => f.write_str("rejected"),
            Self::Revised => f.write_str("revised"),
            Self::Superseded => f.write_str("superseded"),
            Self::Abandoned => f.write_str("abandoned"),
        }
    }
}
impl ::std::str::FromStr for CreateResolutionRequestInputResolutionStatus {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "draft" => Ok(Self::Draft),
            "review" => Ok(Self::Review),
            "proposed" => Ok(Self::Proposed),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "revised" => Ok(Self::Revised),
            "superseded" => Ok(Self::Superseded),
            "abandoned" => Ok(Self::Abandoned),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CreateResolutionRequestInputResolutionStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for CreateResolutionRequestInputResolutionStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for CreateResolutionRequestInputResolutionStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
