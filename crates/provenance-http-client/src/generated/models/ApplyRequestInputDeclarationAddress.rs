// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ApplyRequestInputDeclarationAddress(
    pub ::std::vec::Vec<ApplyRequestInputDeclarationAddressItem>,
);
impl ::std::ops::Deref for ApplyRequestInputDeclarationAddress {
    type Target = ::std::vec::Vec<ApplyRequestInputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<ApplyRequestInputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<ApplyRequestInputDeclarationAddress>
for ::std::vec::Vec<ApplyRequestInputDeclarationAddressItem> {
    fn from(value: ApplyRequestInputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<ApplyRequestInputDeclarationAddressItem>>
for ApplyRequestInputDeclarationAddress {
    fn from(value: ::std::vec::Vec<ApplyRequestInputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
