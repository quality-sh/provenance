// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CreateSourceRequestInputCreateSourceInput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub commit_pin: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub effective_date: ::std::option::Option<i64>,
    pub id: CreateSourceRequestInputStableId,
    pub name: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<CreateSourceRequestInputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<CreateSourceRequestInputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub reference: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub review_date: ::std::option::Option<i64>,
    pub scope_id: CreateSourceRequestInputScopeId,
    pub source_type: CreateSourceRequestInputSourceType,
    pub supersedes: ::std::vec::Vec<CreateSourceRequestInputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub url: ::std::option::Option<::std::string::String>,
}
