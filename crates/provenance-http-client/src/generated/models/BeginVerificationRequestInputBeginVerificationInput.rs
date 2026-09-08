// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BeginVerificationRequestInputBeginVerificationInput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub commit: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub declaration: ::std::option::Option<
        BeginVerificationRequestInputDeclarationReferenceInput,
    >,
    pub declared_by: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub file: ::std::option::Option<::std::string::String>,
    pub key: ::std::string::String,
    ///Semantic validation accepts examples, property, conformance, construction, exhaustion, and proof.
    pub method: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub rule: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub symbol: ::std::option::Option<::std::string::String>,
}
