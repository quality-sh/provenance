// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CreateRequirementSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<CreateRequirementSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for CreateRequirementSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<CreateRequirementSuccessOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<CreateRequirementSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<CreateRequirementSuccessOutputDeclarationAddress>
for ::std::vec::Vec<CreateRequirementSuccessOutputDeclarationAddressItem> {
    fn from(value: CreateRequirementSuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<CreateRequirementSuccessOutputDeclarationAddressItem>,
> for CreateRequirementSuccessOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<CreateRequirementSuccessOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
