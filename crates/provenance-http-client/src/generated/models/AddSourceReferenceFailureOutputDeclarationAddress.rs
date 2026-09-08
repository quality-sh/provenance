// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct AddSourceReferenceFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<AddSourceReferenceFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for AddSourceReferenceFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<AddSourceReferenceFailureOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<AddSourceReferenceFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<AddSourceReferenceFailureOutputDeclarationAddress>
for ::std::vec::Vec<AddSourceReferenceFailureOutputDeclarationAddressItem> {
    fn from(value: AddSourceReferenceFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<AddSourceReferenceFailureOutputDeclarationAddressItem>,
> for AddSourceReferenceFailureOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<AddSourceReferenceFailureOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
