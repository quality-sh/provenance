// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRequirementFailureOutputReconciledResource {
    pub address: CreateRequirementFailureOutputDeclarationAddress,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub changes: ::std::vec::Vec<CreateRequirementFailureOutputTypedFieldChange>,
    pub id: CreateRequirementFailureOutputStableId,
    pub key: ::std::string::String,
    pub kind: CreateRequirementFailureOutputTypedResourceKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub parent: ::std::option::Option<::std::string::String>,
    pub state: CreateRequirementFailureOutputReconcileState,
}
