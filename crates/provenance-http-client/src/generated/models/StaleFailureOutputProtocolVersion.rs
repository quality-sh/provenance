// Generated from OpenAPI. Do not edit.
#[derive(::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct StaleFailureOutputProtocolVersion(i64);
impl ::std::ops::Deref for StaleFailureOutputProtocolVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<StaleFailureOutputProtocolVersion> for i64 {
    fn from(value: StaleFailureOutputProtocolVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for StaleFailureOutputProtocolVersion {
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
impl<'de> ::serde::Deserialize<'de> for StaleFailureOutputProtocolVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| { <D::Error as ::serde::de::Error>::custom(e.to_string()) })
    }
}
