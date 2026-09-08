// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanFailureOutputReconciledResource {
    pub address: PlanFailureOutputDeclarationAddress,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub changes: ::std::vec::Vec<PlanFailureOutputTypedFieldChange>,
    pub id: PlanFailureOutputStableId,
    pub key: ::std::string::String,
    pub kind: PlanFailureOutputTypedResourceKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub parent: ::std::option::Option<::std::string::String>,
    pub state: PlanFailureOutputReconcileState,
}
