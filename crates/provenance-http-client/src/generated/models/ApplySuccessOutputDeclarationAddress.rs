// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ApplySuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<ApplySuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for ApplySuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<ApplySuccessOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<ApplySuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<ApplySuccessOutputDeclarationAddress>
for ::std::vec::Vec<ApplySuccessOutputDeclarationAddressItem> {
    fn from(value: ApplySuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<ApplySuccessOutputDeclarationAddressItem>>
for ApplySuccessOutputDeclarationAddress {
    fn from(value: ::std::vec::Vec<ApplySuccessOutputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
