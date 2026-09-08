// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CreateSourceFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<CreateSourceFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for CreateSourceFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<CreateSourceFailureOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<CreateSourceFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<CreateSourceFailureOutputDeclarationAddress>
for ::std::vec::Vec<CreateSourceFailureOutputDeclarationAddressItem> {
    fn from(value: CreateSourceFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<CreateSourceFailureOutputDeclarationAddressItem>,
> for CreateSourceFailureOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<CreateSourceFailureOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
