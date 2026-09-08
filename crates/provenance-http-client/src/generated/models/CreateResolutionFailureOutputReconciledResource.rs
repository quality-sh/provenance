// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateResolutionFailureOutputReconciledResource {
    pub address: CreateResolutionFailureOutputDeclarationAddress,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub changes: ::std::vec::Vec<CreateResolutionFailureOutputTypedFieldChange>,
    pub id: CreateResolutionFailureOutputStableId,
    pub key: ::std::string::String,
    pub kind: CreateResolutionFailureOutputTypedResourceKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub parent: ::std::option::Option<::std::string::String>,
    pub state: CreateResolutionFailureOutputReconcileState,
}
