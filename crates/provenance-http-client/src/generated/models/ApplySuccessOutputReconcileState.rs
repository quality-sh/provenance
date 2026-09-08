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
pub enum ApplySuccessOutputReconcileState {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "updated")]
    Updated,
    #[serde(rename = "moved")]
    Moved,
    #[serde(rename = "retired")]
    Retired,
    #[serde(rename = "conflict")]
    Conflict,
    #[serde(rename = "unchanged")]
    Unchanged,
}
impl ::std::fmt::Display for ApplySuccessOutputReconcileState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Created => f.write_str("created"),
            Self::Updated => f.write_str("updated"),
            Self::Moved => f.write_str("moved"),
            Self::Retired => f.write_str("retired"),
            Self::Conflict => f.write_str("conflict"),
            Self::Unchanged => f.write_str("unchanged"),
        }
    }
}
impl ::std::str::FromStr for ApplySuccessOutputReconcileState {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "created" => Ok(Self::Created),
            "updated" => Ok(Self::Updated),
            "moved" => Ok(Self::Moved),
            "retired" => Ok(Self::Retired),
            "conflict" => Ok(Self::Conflict),
            "unchanged" => Ok(Self::Unchanged),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ApplySuccessOutputReconcileState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for ApplySuccessOutputReconcileState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for ApplySuccessOutputReconcileState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
