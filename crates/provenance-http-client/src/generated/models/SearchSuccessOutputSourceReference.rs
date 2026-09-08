// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct SearchSuccessOutputSourceReference {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub clause: ::std::option::Option<::std::string::String>,
    pub source_id: SearchSuccessOutputStableId,
}
