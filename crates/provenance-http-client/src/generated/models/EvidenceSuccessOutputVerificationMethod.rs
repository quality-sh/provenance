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
pub enum EvidenceSuccessOutputVerificationMethod {
    #[serde(rename = "exhaustion")]
    Exhaustion,
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "examples")]
    Examples,
    #[serde(rename = "conformance")]
    Conformance,
    #[serde(rename = "construction")]
    Construction,
    #[serde(rename = "proof")]
    Proof,
}
impl ::std::fmt::Display for EvidenceSuccessOutputVerificationMethod {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Exhaustion => f.write_str("exhaustion"),
            Self::Property => f.write_str("property"),
            Self::Examples => f.write_str("examples"),
            Self::Conformance => f.write_str("conformance"),
            Self::Construction => f.write_str("construction"),
            Self::Proof => f.write_str("proof"),
        }
    }
}
impl ::std::str::FromStr for EvidenceSuccessOutputVerificationMethod {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "exhaustion" => Ok(Self::Exhaustion),
            "property" => Ok(Self::Property),
            "examples" => Ok(Self::Examples),
            "conformance" => Ok(Self::Conformance),
            "construction" => Ok(Self::Construction),
            "proof" => Ok(Self::Proof),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EvidenceSuccessOutputVerificationMethod {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for EvidenceSuccessOutputVerificationMethod {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for EvidenceSuccessOutputVerificationMethod {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
