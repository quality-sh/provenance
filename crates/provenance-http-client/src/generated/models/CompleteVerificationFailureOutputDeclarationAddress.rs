// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CompleteVerificationFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<CompleteVerificationFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for CompleteVerificationFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<
        CompleteVerificationFailureOutputDeclarationAddressItem,
    >;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<CompleteVerificationFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<CompleteVerificationFailureOutputDeclarationAddress>
for ::std::vec::Vec<CompleteVerificationFailureOutputDeclarationAddressItem> {
    fn from(value: CompleteVerificationFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<CompleteVerificationFailureOutputDeclarationAddressItem>,
> for CompleteVerificationFailureOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<CompleteVerificationFailureOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
