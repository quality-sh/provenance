// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct AddSourceReferenceRequestInputAddSourceReferenceInput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub clause: ::std::option::Option<::std::string::String>,
    pub requirement_id: AddSourceReferenceRequestInputStableId,
    pub scope_id: AddSourceReferenceRequestInputScopeId,
    pub source_id: AddSourceReferenceRequestInputStableId,
}
