// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct InfoSuccessOutput {
    pub engine_version: ::std::string::String,
    pub protocol_version: InfoSuccessOutputProtocolVersion,
    pub repository: ::std::string::String,
    pub state_schema_version: u32,
}
