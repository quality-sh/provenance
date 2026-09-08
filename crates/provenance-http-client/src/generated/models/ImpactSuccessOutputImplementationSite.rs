// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ImpactSuccessOutputImplementationSite {
    pub file: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub line: ::std::option::Option<u32>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub symbol: ::std::option::Option<::std::string::String>,
}
