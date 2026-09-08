// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanSuccessOutputReconciledResource {
    pub address: PlanSuccessOutputDeclarationAddress,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub changes: ::std::vec::Vec<PlanSuccessOutputTypedFieldChange>,
    pub id: PlanSuccessOutputStableId,
    pub key: ::std::string::String,
    pub kind: PlanSuccessOutputTypedResourceKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub parent: ::std::option::Option<::std::string::String>,
    pub state: PlanSuccessOutputReconcileState,
}
