// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ApplySuccessOutput {
    pub conflicts: u32,
    pub created: u32,
    pub declared_by: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub diagnostics: ::std::vec::Vec<ApplySuccessOutputTypedSpecDiagnostic>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub implementation_bindings: ::std::vec::Vec<
        ApplySuccessOutputImplementationBinding,
    >,
    pub moved: u32,
    pub resources: ::std::vec::Vec<ApplySuccessOutputReconciledResource>,
    pub retired: u32,
    pub unchanged: u32,
    pub updated: u32,
}
