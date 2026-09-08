// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PlanSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<PlanSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for PlanSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<PlanSuccessOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<PlanSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<PlanSuccessOutputDeclarationAddress>
for ::std::vec::Vec<PlanSuccessOutputDeclarationAddressItem> {
    fn from(value: PlanSuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<PlanSuccessOutputDeclarationAddressItem>>
for PlanSuccessOutputDeclarationAddress {
    fn from(value: ::std::vec::Vec<PlanSuccessOutputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
