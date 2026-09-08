// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PlanRequestInputDeclarationAddress(
    pub ::std::vec::Vec<PlanRequestInputDeclarationAddressItem>,
);
impl ::std::ops::Deref for PlanRequestInputDeclarationAddress {
    type Target = ::std::vec::Vec<PlanRequestInputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<PlanRequestInputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<PlanRequestInputDeclarationAddress>
for ::std::vec::Vec<PlanRequestInputDeclarationAddressItem> {
    fn from(value: PlanRequestInputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<PlanRequestInputDeclarationAddressItem>>
for PlanRequestInputDeclarationAddress {
    fn from(value: ::std::vec::Vec<PlanRequestInputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
