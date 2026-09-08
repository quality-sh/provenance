// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ImpactSuccessOutput {
    pub affected_rules: ::std::vec::Vec<ImpactSuccessOutputAffectedRule>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_cause: ::std::option::Option<ImpactSuccessOutputFreshnessCause>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_error: ::std::option::Option<::std::string::String>,
    pub has_more: bool,
    pub id: ::std::string::String,
    pub limit: u32,
    pub operation: ImpactSuccessOutputOperation,
    pub protocol_version: ImpactSuccessOutputProtocolVersion,
    /**The working-tree scan stopped at the configured file count, so the
scanned sites are a lower bound.*/
    pub scan_cut: bool,
    pub stamp: ImpactSuccessOutputStamp,
}
