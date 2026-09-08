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
pub enum StaleSuccessOutputStampPolicy {
    /**Catch-up ran under the publication guard and the answer is at or
after the serial it committed.*/
    #[serde(rename = "catch_up")]
    CatchUp,
    ///No freshness step; the answer is at the stored serial.
    #[serde(rename = "annotate_only")]
    AnnotateOnly,
    ///Reserved: a read refuses when the projection is behind.
    #[serde(rename = "refuse_stale")]
    RefuseStale,
    /**Catch-up failed and the answer is at the stored serial; the error
text travels in `freshness_error`.*/
    #[serde(rename = "catch_up_failed")]
    CatchUpFailed,
}
impl ::std::fmt::Display for StaleSuccessOutputStampPolicy {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CatchUp => f.write_str("catch_up"),
            Self::AnnotateOnly => f.write_str("annotate_only"),
            Self::RefuseStale => f.write_str("refuse_stale"),
            Self::CatchUpFailed => f.write_str("catch_up_failed"),
        }
    }
}
impl ::std::str::FromStr for StaleSuccessOutputStampPolicy {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "catch_up" => Ok(Self::CatchUp),
            "annotate_only" => Ok(Self::AnnotateOnly),
            "refuse_stale" => Ok(Self::RefuseStale),
            "catch_up_failed" => Ok(Self::CatchUpFailed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for StaleSuccessOutputStampPolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for StaleSuccessOutputStampPolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StaleSuccessOutputStampPolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
