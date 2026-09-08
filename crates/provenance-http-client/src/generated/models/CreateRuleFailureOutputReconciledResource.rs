// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRuleFailureOutputReconciledResource {
    pub address: CreateRuleFailureOutputDeclarationAddress,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub changes: ::std::vec::Vec<CreateRuleFailureOutputTypedFieldChange>,
    pub id: CreateRuleFailureOutputStableId,
    pub key: ::std::string::String,
    pub kind: CreateRuleFailureOutputTypedResourceKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub parent: ::std::option::Option<::std::string::String>,
    pub state: CreateRuleFailureOutputReconcileState,
}
