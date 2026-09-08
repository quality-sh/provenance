// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum GetFailureOutputReadFailure {
    #[serde(rename = "no_projection")]
    NoProjection,
    #[serde(rename = "stale")]
    Stale {
        digest: ::std::string::String,
        instance_id: ::std::string::String,
        moved: ::std::vec::Vec<GetFailureOutputMovedUnit>,
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
