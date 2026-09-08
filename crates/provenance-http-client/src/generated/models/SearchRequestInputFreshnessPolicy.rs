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
pub enum SearchRequestInputFreshnessPolicy {
    ///Run catch-up under the publication guard, then answer.
    #[serde(rename = "catch_up")]
    CatchUp,
    ///Answer at the stored serial without a freshness step.
    #[serde(rename = "annotate_only")]
    AnnotateOnly,
    ///Refuse when the stored projection differs from canonical state.
    #[serde(rename = "refuse_stale")]
    RefuseStale,
}
impl ::std::fmt::Display for SearchRequestInputFreshnessPolicy {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CatchUp => f.write_str("catch_up"),
            Self::AnnotateOnly => f.write_str("annotate_only"),
            Self::RefuseStale => f.write_str("refuse_stale"),
        }
    }
}
impl ::std::str::FromStr for SearchRequestInputFreshnessPolicy {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "catch_up" => Ok(Self::CatchUp),
            "annotate_only" => Ok(Self::AnnotateOnly),
            "refuse_stale" => Ok(Self::RefuseStale),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SearchRequestInputFreshnessPolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for SearchRequestInputFreshnessPolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for SearchRequestInputFreshnessPolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
