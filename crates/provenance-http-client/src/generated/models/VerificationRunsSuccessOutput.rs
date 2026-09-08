// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct VerificationRunsSuccessOutput(
    pub ::std::vec::Vec<VerificationRunsSuccessOutputVerificationRun>,
);
impl ::std::ops::Deref for VerificationRunsSuccessOutput {
    type Target = ::std::vec::Vec<VerificationRunsSuccessOutputVerificationRun>;
    fn deref(&self) -> &::std::vec::Vec<VerificationRunsSuccessOutputVerificationRun> {
        &self.0
    }
}
impl ::std::convert::From<VerificationRunsSuccessOutput>
for ::std::vec::Vec<VerificationRunsSuccessOutputVerificationRun> {
    fn from(value: VerificationRunsSuccessOutput) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<VerificationRunsSuccessOutputVerificationRun>>
for VerificationRunsSuccessOutput {
    fn from(
        value: ::std::vec::Vec<VerificationRunsSuccessOutputVerificationRun>,
    ) -> Self {
        Self(value)
    }
}
