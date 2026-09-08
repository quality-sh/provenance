// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolSuccessOutputStamp {
    pub attested: ::std::vec::Vec<::std::string::String>,
    /**The reader logic version. It moves when the reader answers
differently over the same rows, never for a migration.*/
    pub derivation: u32,
    pub digest: ::std::string::String,
    pub instance_id: ::std::string::String,
    pub live: ::std::vec::Vec<::std::string::String>,
    pub policy: ResolveSymbolSuccessOutputStampPolicy,
    pub serial: i64,
}
