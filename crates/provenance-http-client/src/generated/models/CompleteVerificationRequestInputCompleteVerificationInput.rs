// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CompleteVerificationRequestInputCompleteVerificationInput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<::std::string::String>,
    pub run: ::std::string::String,
    ///Semantic validation accepts passed or failed.
    pub status: ::std::string::String,
}
