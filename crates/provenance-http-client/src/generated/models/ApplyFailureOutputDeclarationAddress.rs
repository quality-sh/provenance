// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ApplyFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<ApplyFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for ApplyFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<ApplyFailureOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<ApplyFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<ApplyFailureOutputDeclarationAddress>
for ::std::vec::Vec<ApplyFailureOutputDeclarationAddressItem> {
    fn from(value: ApplyFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<ApplyFailureOutputDeclarationAddressItem>>
for ApplyFailureOutputDeclarationAddress {
    fn from(value: ::std::vec::Vec<ApplyFailureOutputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
