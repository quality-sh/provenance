// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct BeginVerificationFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<BeginVerificationFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for BeginVerificationFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<BeginVerificationFailureOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<BeginVerificationFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<BeginVerificationFailureOutputDeclarationAddress>
for ::std::vec::Vec<BeginVerificationFailureOutputDeclarationAddressItem> {
    fn from(value: BeginVerificationFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<BeginVerificationFailureOutputDeclarationAddressItem>,
> for BeginVerificationFailureOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<BeginVerificationFailureOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
