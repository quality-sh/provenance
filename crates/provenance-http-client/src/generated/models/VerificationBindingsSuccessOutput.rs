// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct VerificationBindingsSuccessOutput(
    pub ::std::vec::Vec<VerificationBindingsSuccessOutputVerificationBinding>,
);
impl ::std::ops::Deref for VerificationBindingsSuccessOutput {
    type Target = ::std::vec::Vec<VerificationBindingsSuccessOutputVerificationBinding>;
    fn deref(
        &self,
    ) -> &::std::vec::Vec<VerificationBindingsSuccessOutputVerificationBinding> {
        &self.0
    }
}
impl ::std::convert::From<VerificationBindingsSuccessOutput>
for ::std::vec::Vec<VerificationBindingsSuccessOutputVerificationBinding> {
    fn from(value: VerificationBindingsSuccessOutput) -> Self {
        value.0
    }
}
impl ::std::convert::From<
    ::std::vec::Vec<VerificationBindingsSuccessOutputVerificationBinding>,
> for VerificationBindingsSuccessOutput {
    fn from(
        value: ::std::vec::Vec<VerificationBindingsSuccessOutputVerificationBinding>,
    ) -> Self {
        Self(value)
    }
}
