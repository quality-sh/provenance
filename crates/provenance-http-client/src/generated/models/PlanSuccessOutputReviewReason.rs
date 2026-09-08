// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanSuccessOutputReviewReason {
    pub after: ::std::string::String,
    pub before: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub changed_at: ::std::option::Option<i64>,
    pub field: ::std::string::String,
    pub requirement: PlanSuccessOutputStableId,
}
