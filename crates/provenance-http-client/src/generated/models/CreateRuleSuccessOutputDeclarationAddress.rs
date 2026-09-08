// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct CreateRuleSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<CreateRuleSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for CreateRuleSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<CreateRuleSuccessOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<CreateRuleSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<CreateRuleSuccessOutputDeclarationAddress>
for ::std::vec::Vec<CreateRuleSuccessOutputDeclarationAddressItem> {
    fn from(value: CreateRuleSuccessOutputDeclarationAddress) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<CreateRuleSuccessOutputDeclarationAddressItem>>
for CreateRuleSuccessOutputDeclarationAddress {
    fn from(
        value: ::std::vec::Vec<CreateRuleSuccessOutputDeclarationAddressItem>,
    ) -> Self {
        Self(value)
    }
}
