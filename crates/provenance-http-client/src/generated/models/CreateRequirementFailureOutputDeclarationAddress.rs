// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CreateRequirementFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<CreateRequirementFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for CreateRequirementFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<CreateRequirementFailureOutputDeclarationAddressItem>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<CreateRequirementFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<CreateRequirementFailureOutputDeclarationAddress>
for ::std::vec::Vec<CreateRequirementFailureOutputDeclarationAddressItem> {
    fn from(value: CreateRequirementFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<CreateRequirementFailureOutputDeclarationAddressItem>,
> for CreateRequirementFailureOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<CreateRequirementFailureOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
