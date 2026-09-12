//! Recoverable Requirement edits and immutable evidence.
mod classifier;
pub(crate) mod guard;
mod input;
mod journal;
mod relationships;
mod save;
pub use input::{RequirementRelations, SaveRequirement};

fn owner_matches(record: &provenance_core::Requirement, owner: Option<&str>) -> anyhow::Result<()> {
    if record.declared_by.as_deref() != owner {
        return Err(crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::RecordOwnershipConflict,
            anyhow::anyhow!("declared_by must match the existing owner"),
        ));
    }
    Ok(())
}

pub(crate) mod cache;

mod reads;
pub use reads::{read_evidence, read_history};

mod snapshot;

#[cfg(test)]
mod recovery_tests;

#[cfg(all(test, any(unix, windows)))]
mod path_tests;

mod discussion_input;
pub use discussion_input::{DiscussionAction, WriteDiscussion};
mod discussion_state;
mod discussion_writes;

mod create;
pub use create::CreateReviewRequirement;
mod discussion_messages;
mod discussion_reads;
pub use discussion_messages::read_discussion_messages;
pub use discussion_reads::read_discussions;
#[cfg(test)]
mod discussion_recovery_tests;

mod decision_input;
pub use decision_input::{
    DecideRequirementReview, ReviewFeedback, SubmitRequirementReview, WithdrawRequirementReview,
};
mod decision;
mod decision_reads;
mod decision_state;
