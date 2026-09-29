//! Recoverable Requirement edits and immutable evidence.
mod classifier;
pub(crate) mod guard;
mod input;
mod journal;
pub(crate) mod relationships;
mod resource_read;
pub(crate) use resource_read::RequirementResourceSnapshot;
mod save;
pub use input::{ListEdit, RequirementRelations, SaveRequirement};

fn owner_matches(record: &impl serde::Serialize, owner: Option<&str>) -> anyhow::Result<()> {
    let value = serde_json::to_value(record)?;
    let declared_by = value
        .get("declared_by")
        .and_then(serde_json::Value::as_str);
    if declared_by != owner {
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
mod concurrency_tests;
#[cfg(test)]
mod recovery_tests;

#[cfg(all(test, any(unix, windows)))]
mod path_tests;

mod discussion_input;
pub use discussion_input::{DiscussionAction, WriteDiscussion};
mod discussion_target;
pub use discussion_target::TargetDiscussionWrite;
mod discussion_state;
mod discussion_writes;

mod create;
pub use create::CreateReviewRequirement;
mod authoring;
mod discussion_discovery;
mod discussion_messages;
mod discussion_page;
mod discussion_reads;
mod typed_adoption;
pub use discussion_discovery::{read_discussion_conversation, read_discussion_list};
pub use discussion_messages::{read_discussion_message, read_discussion_messages};
pub use discussion_reads::{read_discussion, read_discussions};
#[cfg(test)]
mod discussion_recovery_tests;

mod decision_input;
pub use decision_input::{
    DecideRecordReview, DecideRequirementReview, ReviewFeedback, SubmitRecordReview,
    SubmitRequirementReview, WithdrawRecordReview, WithdrawRequirementReview,
};
mod automatic_submission;
mod decision;
mod decision_reads;
mod decision_state;

#[cfg(test)]
mod decision_tests;
