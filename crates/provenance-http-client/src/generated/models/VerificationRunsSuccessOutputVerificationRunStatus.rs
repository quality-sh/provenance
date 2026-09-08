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
pub enum VerificationRunsSuccessOutputVerificationRunStatus {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "passed")]
    Passed,
    #[serde(rename = "failed")]
    Failed,
}
impl ::std::fmt::Display for VerificationRunsSuccessOutputVerificationRunStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Running => f.write_str("running"),
            Self::Passed => f.write_str("passed"),
            Self::Failed => f.write_str("failed"),
        }
    }
}
impl ::std::str::FromStr for VerificationRunsSuccessOutputVerificationRunStatus {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "running" => Ok(Self::Running),
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str>
for VerificationRunsSuccessOutputVerificationRunStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for VerificationRunsSuccessOutputVerificationRunStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for VerificationRunsSuccessOutputVerificationRunStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
