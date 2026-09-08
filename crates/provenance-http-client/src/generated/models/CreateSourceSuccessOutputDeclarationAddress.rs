// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CreateSourceSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<CreateSourceSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for CreateSourceSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<CreateSourceSuccessOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<CreateSourceSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<CreateSourceSuccessOutputDeclarationAddress>
for ::std::vec::Vec<CreateSourceSuccessOutputDeclarationAddressItem> {
    fn from(value: CreateSourceSuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<CreateSourceSuccessOutputDeclarationAddressItem>,
> for CreateSourceSuccessOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<CreateSourceSuccessOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
