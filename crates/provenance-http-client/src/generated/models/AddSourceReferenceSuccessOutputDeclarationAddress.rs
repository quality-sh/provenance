// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct AddSourceReferenceSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<AddSourceReferenceSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for AddSourceReferenceSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<AddSourceReferenceSuccessOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<AddSourceReferenceSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<AddSourceReferenceSuccessOutputDeclarationAddress>
for ::std::vec::Vec<AddSourceReferenceSuccessOutputDeclarationAddressItem> {
    fn from(value: AddSourceReferenceSuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<AddSourceReferenceSuccessOutputDeclarationAddressItem>,
> for AddSourceReferenceSuccessOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<AddSourceReferenceSuccessOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
