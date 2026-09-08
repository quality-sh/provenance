// Generated from OpenAPI. Do not edit.
#[derive(::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct EvidenceFailureOutputProtocolVersion(i64);
impl ::std::ops::Deref for EvidenceFailureOutputProtocolVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EvidenceFailureOutputProtocolVersion> for i64 {
    fn from(value: EvidenceFailureOutputProtocolVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EvidenceFailureOutputProtocolVersion {
    type Error = self::error::ConversionError;
    fn try_from(
        value: i64,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![7_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EvidenceFailureOutputProtocolVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| { <D::Error as ::serde::de::Error>::custom(e.to_string()) })
    }
}
