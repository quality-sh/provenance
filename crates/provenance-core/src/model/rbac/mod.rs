mod policy;
mod types;

pub use policy::{
    authorize, ensure_disposition_actor_is_human, ensure_rbac_section_well_formed,
    ensure_unambiguous_rbac, AMBIGUOUS_MANIFEST_REFUSAL, MISSING_CLAIM_REFUSAL,
    RATIFICATION_REFUSAL_TAIL,
};
pub use types::{Assignment, Capability, RbacClaim, RbacResource, RbacSection};

#[cfg(test)]
mod tests;
