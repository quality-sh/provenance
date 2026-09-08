// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PlanFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<PlanFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for PlanFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<PlanFailureOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<PlanFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<PlanFailureOutputDeclarationAddress>
for ::std::vec::Vec<PlanFailureOutputDeclarationAddressItem> {
    fn from(value: PlanFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<PlanFailureOutputDeclarationAddressItem>>
for PlanFailureOutputDeclarationAddress {
    fn from(value: ::std::vec::Vec<PlanFailureOutputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
