// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ImpactSuccessOutputAffectedRule {
    pub id: ImpactSuccessOutputStableId,
    pub implementations: ::std::vec::Vec<ImpactSuccessOutputImplementationSite>,
    pub verifications: ::std::vec::Vec<ImpactSuccessOutputVerificationSite>,
}
