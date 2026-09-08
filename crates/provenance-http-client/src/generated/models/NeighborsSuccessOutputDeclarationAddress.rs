// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct NeighborsSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<NeighborsSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for NeighborsSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<NeighborsSuccessOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<NeighborsSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<NeighborsSuccessOutputDeclarationAddress>
for ::std::vec::Vec<NeighborsSuccessOutputDeclarationAddressItem> {
    fn from(value: NeighborsSuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<NeighborsSuccessOutputDeclarationAddressItem>>
for NeighborsSuccessOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<NeighborsSuccessOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
