// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CreateRuleFailureOutputDeclarationAddress(
    pub ::std::vec::Vec<CreateRuleFailureOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for CreateRuleFailureOutputDeclarationAddress {
    type Target = ::std::vec::Vec<CreateRuleFailureOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<CreateRuleFailureOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<CreateRuleFailureOutputDeclarationAddress>
for ::std::vec::Vec<CreateRuleFailureOutputDeclarationAddressItem> {
    fn from(value: CreateRuleFailureOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<CreateRuleFailureOutputDeclarationAddressItem>>
for CreateRuleFailureOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<CreateRuleFailureOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
