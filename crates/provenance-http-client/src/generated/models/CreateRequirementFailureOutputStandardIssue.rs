// Generated from OpenAPI. Do not edit.
#[derive(::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CreateRequirementFailureOutputStandardIssue(u8);
impl ::std::ops::Deref for CreateRequirementFailureOutputStandardIssue {
    type Target = u8;
    fn deref(&self) -> &u8 {
        &self.0
    }
}
impl ::std::convert::From<CreateRequirementFailureOutputStandardIssue> for u8 {
    fn from(value: CreateRequirementFailureOutputStandardIssue) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<u8> for CreateRequirementFailureOutputStandardIssue {
    type Error = self::error::ConversionError;
    fn try_from(value: u8) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![9_u8].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CreateRequirementFailureOutputStandardIssue {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<u8>::deserialize(deserializer)?)
            .map_err(|e| { <D::Error as ::serde::de::Error>::custom(e.to_string()) })
    }
}
