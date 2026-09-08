// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ResolveSymbolSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<ResolveSymbolSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for ResolveSymbolSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<ResolveSymbolSuccessOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<ResolveSymbolSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<ResolveSymbolSuccessOutputDeclarationAddress>
for ::std::vec::Vec<ResolveSymbolSuccessOutputDeclarationAddressItem> {
    fn from(value: ResolveSymbolSuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<ResolveSymbolSuccessOutputDeclarationAddressItem>,
> for ResolveSymbolSuccessOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<ResolveSymbolSuccessOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
