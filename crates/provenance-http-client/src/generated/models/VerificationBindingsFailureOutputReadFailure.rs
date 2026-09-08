// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum VerificationBindingsFailureOutputReadFailure {
    #[serde(rename = "file_access_denied")]
    FileAccessDenied,
    #[serde(rename = "file_unavailable")]
    FileUnavailable,
    #[serde(rename = "git_unavailable")]
    GitUnavailable,
    #[serde(rename = "git_revision_not_found")]
    GitRevisionNotFound,
    #[serde(rename = "no_projection")]
    NoProjection,
    #[serde(rename = "stale")]
    Stale {
        digest: ::std::string::String,
        instance_id: ::std::string::String,
        moved: ::std::vec::Vec<VerificationBindingsFailureOutputMovedUnit>,
        serial: i64,
    },
    #[serde(rename = "unit_unreadable")]
    UnitUnreadable { unit: ::std::string::String },
    #[serde(rename = "schema_behind")]
    SchemaBehind,
    #[serde(rename = "half_migrated")]
    HalfMigrated,
    #[serde(rename = "read_failed")]
    ReadFailed,
}
