// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BeginVerificationRequestInputDeclarationReferenceInput {
    pub address: BeginVerificationRequestInputDeclarationAddress,
    pub declared_by: ::std::string::String,
}
