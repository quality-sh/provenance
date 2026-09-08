// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct SearchSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<SearchSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for SearchSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<SearchSuccessOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<SearchSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<SearchSuccessOutputDeclarationAddress>
for ::std::vec::Vec<SearchSuccessOutputDeclarationAddressItem> {
    fn from(value: SearchSuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<SearchSuccessOutputDeclarationAddressItem>>
for SearchSuccessOutputDeclarationAddress {
    fn from(value: ::std::vec::Vec<SearchSuccessOutputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
