// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct AddSourceReferenceFailureOutputReconciledResource {
    pub address: AddSourceReferenceFailureOutputDeclarationAddress,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub changes: ::std::vec::Vec<AddSourceReferenceFailureOutputTypedFieldChange>,
    pub id: AddSourceReferenceFailureOutputStableId,
    pub key: ::std::string::String,
    pub kind: AddSourceReferenceFailureOutputTypedResourceKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub parent: ::std::option::Option<::std::string::String>,
    pub state: AddSourceReferenceFailureOutputReconcileState,
}
