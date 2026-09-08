// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct TraceSuccessOutputDeclarationAddress(
    pub ::std::vec::Vec<TraceSuccessOutputDeclarationAddressItem>,
);
impl ::std::ops::Deref for TraceSuccessOutputDeclarationAddress {
    type Target = ::std::vec::Vec<TraceSuccessOutputDeclarationAddressItem>;
    fn deref(&self) -> &::std::vec::Vec<TraceSuccessOutputDeclarationAddressItem> {
        &self.0
    }
}
impl ::std::convert::From<::std::vec::Vec<TraceSuccessOutputDeclarationAddressItem>>
for TraceSuccessOutputDeclarationAddress {
    fn from(value: ::std::vec::Vec<TraceSuccessOutputDeclarationAddressItem>) -> Self {
        Self(value)
    }
}
