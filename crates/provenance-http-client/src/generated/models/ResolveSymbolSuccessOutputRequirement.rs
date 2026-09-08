// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolSuccessOutputRequirement {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub declaration_address: ::std::option::Option<
        ResolveSymbolSuccessOutputDeclarationAddress,
    >,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub declared_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub depends_on: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub domain_id: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    /**Deliberately unstructured free text: the dim view of decisions and
investigations that are coming but cannot yet be phrased sharply.*/
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub fog: ::std::option::Option<::std::string::String>,
    pub id: ResolveSymbolSuccessOutputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub refines: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub retired: ::std::option::Option<bool>,
    pub schema_version: u32,
    pub scope_id: ResolveSymbolSuccessOutputScopeId,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub source_refs: ::std::vec::Vec<ResolveSymbolSuccessOutputSourceReference>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub spawned_by: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    pub statement: ::std::string::String,
    pub status: ResolveSymbolSuccessOutputRequirementStatus,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub supersedes: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
}
