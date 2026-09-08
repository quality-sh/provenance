// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct BeginVerificationRequestInputDeclarationAddress(
    pub ::std::vec::Vec<BeginVerificationRequestInputDeclarationAddressItem>,
);
impl ::std::ops::Deref for BeginVerificationRequestInputDeclarationAddress {
    type Target = ::std::vec::Vec<BeginVerificationRequestInputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<BeginVerificationRequestInputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<BeginVerificationRequestInputDeclarationAddress>
for ::std::vec::Vec<BeginVerificationRequestInputDeclarationAddressItem> {
    fn from(value: BeginVerificationRequestInputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<BeginVerificationRequestInputDeclarationAddressItem>,
> for BeginVerificationRequestInputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<BeginVerificationRequestInputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
