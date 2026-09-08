// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct GetSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<GetSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for GetSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<GetSuccessOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<GetSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<::std::vec::Vec<GetSuccessOutputDeclarationAddressItem>>
for GetSuccessOutputDeclarationAddress {
    fn from(value: ::std::vec::Vec<GetSuccessOutputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
