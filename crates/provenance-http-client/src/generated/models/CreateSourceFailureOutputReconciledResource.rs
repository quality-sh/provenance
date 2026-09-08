// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateSourceFailureOutputReconciledResource {
    pub address: CreateSourceFailureOutputDeclarationAddress,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub changes: ::std::vec::Vec<CreateSourceFailureOutputTypedFieldChange>,
    pub id: CreateSourceFailureOutputStableId,
    pub key: ::std::string::String,
    pub kind: CreateSourceFailureOutputTypedResourceKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub parent: ::std::option::Option<::std::string::String>,
    pub state: CreateSourceFailureOutputReconcileState,
}
