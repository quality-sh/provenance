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
pub enum TraceSuccessOutputSourceType {
    #[serde(rename = "policy")]
    Policy,
    #[serde(rename = "document")]
    Document,
    #[serde(rename = "legislation")]
    Legislation,
    #[serde(rename = "company_agreement")]
    CompanyAgreement,
    #[serde(rename = "system_state")]
    SystemState,
    #[serde(rename = "external_integration")]
    ExternalIntegration,
    #[serde(rename = "domain_knowledge")]
    DomainKnowledge,
    #[serde(rename = "project_artifact")]
    ProjectArtifact,
    #[serde(rename = "incident")]
    Incident,
    #[serde(rename = "api_spec")]
    ApiSpec,
}
impl ::std::fmt::Display for TraceSuccessOutputSourceType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Policy => f.write_str("policy"),
            Self::Document => f.write_str("document"),
            Self::Legislation => f.write_str("legislation"),
            Self::CompanyAgreement => f.write_str("company_agreement"),
            Self::SystemState => f.write_str("system_state"),
            Self::ExternalIntegration => f.write_str("external_integration"),
            Self::DomainKnowledge => f.write_str("domain_knowledge"),
            Self::ProjectArtifact => f.write_str("project_artifact"),
            Self::Incident => f.write_str("incident"),
            Self::ApiSpec => f.write_str("api_spec"),
        }
    }
}
impl ::std::str::FromStr for TraceSuccessOutputSourceType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "policy" => Ok(Self::Policy),
            "document" => Ok(Self::Document),
            "legislation" => Ok(Self::Legislation),
            "company_agreement" => Ok(Self::CompanyAgreement),
            "system_state" => Ok(Self::SystemState),
            "external_integration" => Ok(Self::ExternalIntegration),
            "domain_knowledge" => Ok(Self::DomainKnowledge),
            "project_artifact" => Ok(Self::ProjectArtifact),
            "incident" => Ok(Self::Incident),
            "api_spec" => Ok(Self::ApiSpec),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TraceSuccessOutputSourceType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for TraceSuccessOutputSourceType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TraceSuccessOutputSourceType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
