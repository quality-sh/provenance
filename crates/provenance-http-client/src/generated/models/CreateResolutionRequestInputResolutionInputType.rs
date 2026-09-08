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
pub enum CreateResolutionRequestInputResolutionInputType {
    #[serde(rename = "regulatory")]
    Regulatory,
    #[serde(rename = "legal_advice")]
    LegalAdvice,
    #[serde(rename = "commercial")]
    Commercial,
    #[serde(rename = "benchmark")]
    Benchmark,
    #[serde(rename = "technical")]
    Technical,
    #[serde(rename = "incident")]
    Incident,
    #[serde(rename = "source_material")]
    SourceMaterial,
}
impl ::std::fmt::Display for CreateResolutionRequestInputResolutionInputType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Regulatory => f.write_str("regulatory"),
            Self::LegalAdvice => f.write_str("legal_advice"),
            Self::Commercial => f.write_str("commercial"),
            Self::Benchmark => f.write_str("benchmark"),
            Self::Technical => f.write_str("technical"),
            Self::Incident => f.write_str("incident"),
            Self::SourceMaterial => f.write_str("source_material"),
        }
    }
}
impl ::std::str::FromStr for CreateResolutionRequestInputResolutionInputType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "regulatory" => Ok(Self::Regulatory),
            "legal_advice" => Ok(Self::LegalAdvice),
            "commercial" => Ok(Self::Commercial),
            "benchmark" => Ok(Self::Benchmark),
            "technical" => Ok(Self::Technical),
            "incident" => Ok(Self::Incident),
            "source_material" => Ok(Self::SourceMaterial),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CreateResolutionRequestInputResolutionInputType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for CreateResolutionRequestInputResolutionInputType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for CreateResolutionRequestInputResolutionInputType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
