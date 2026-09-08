// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct BeginVerificationFailureOutputReconciledResource {
    pub address: BeginVerificationFailureOutputDeclarationAddress,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub changes: ::std::vec::Vec<BeginVerificationFailureOutputTypedFieldChange>,
    pub id: BeginVerificationFailureOutputStableId,
    pub key: ::std::string::String,
    pub kind: BeginVerificationFailureOutputTypedResourceKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub parent: ::std::option::Option<::std::string::String>,
    pub state: BeginVerificationFailureOutputReconcileState,
}
