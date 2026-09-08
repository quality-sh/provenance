// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CreateResolutionFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<CreateResolutionFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for CreateResolutionFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<CreateResolutionFailureOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<CreateResolutionFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<CreateResolutionFailureOutputDeclarationAddress>
for ::std::vec::Vec<CreateResolutionFailureOutputDeclarationAddressItem> {
    fn from(value: CreateResolutionFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<CreateResolutionFailureOutputDeclarationAddressItem>,
> for CreateResolutionFailureOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<CreateResolutionFailureOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
