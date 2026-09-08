// Generated from OpenAPI. Do not edit.
#[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct CompleteVerificationFailureOutputDeclarationAddressItem(
    ::std::string::String,
);
impl ::std::ops::Deref for CompleteVerificationFailureOutputDeclarationAddressItem {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CompleteVerificationFailureOutputDeclarationAddressItem>
for ::std::string::String {
    fn from(value: CompleteVerificationFailureOutputDeclarationAddressItem) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for CompleteVerificationFailureOutputDeclarationAddressItem {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> = ::std::sync::LazyLock::new(||
        { ::regress::Regex::new("\\S").unwrap() });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"\\S\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str>
for CompleteVerificationFailureOutputDeclarationAddressItem {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
for CompleteVerificationFailureOutputDeclarationAddressItem {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
for CompleteVerificationFailureOutputDeclarationAddressItem {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de>
for CompleteVerificationFailureOutputDeclarationAddressItem {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
