//! Guarded graph record edits and their review state.
mod classifier;
pub(crate) mod guard;
mod input;
mod publication;
pub(crate) mod relationships;
mod resource_read;
pub(crate) use resource_read::RequirementResourceSnapshot;
mod save;
pub use input::{ListEdit, RequirementRelations, SaveRequirement};

/// Creates the caller-independent identity for a review write.
#[provenance_macros::rule("rule_review_request_identity_server_created")]
fn new_id() -> provenance_core::StableId {
    provenance_core::StableId::new(uuid::Uuid::new_v4().to_string())
        .expect("UUID uses valid stable ID characters")
}

fn owner_matches(record: &impl serde::Serialize, owner: Option<&str>) -> anyhow::Result<()> {
    let value = serde_json::to_value(record)?;
    let declared_by = value.get("declared_by").and_then(serde_json::Value::as_str);
    if declared_by != owner {
        return Err(crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::RecordOwnershipConflict,
            anyhow::anyhow!("declared_by must match the existing owner"),
        ));
    }
    Ok(())
}

mod history;
mod reads;
pub use reads::{read_evidence, read_history};

#[cfg(test)]
mod concurrency_tests;
#[cfg(test)]
mod recovery_tests;

mod discussion_input;
pub use discussion_input::{DiscussionAction, WriteDiscussion};
mod discussion_target;
pub use discussion_target::TargetDiscussionWrite;
mod discussion_state;
mod discussion_writes;

mod create;
pub use create::CreateReviewRequirement;
mod authoring;
mod native_batch;
pub(crate) use native_batch::NativeRecordBatch;
mod discussion_discovery;
mod discussion_messages;
mod discussion_page;
mod discussion_reads;
mod typed_adoption;
#[cfg(test)]
mod typed_adoption_tests;
pub use discussion_discovery::{read_discussion_conversation, read_discussion_list};
pub use discussion_messages::{read_discussion_message, read_discussion_messages};
pub use discussion_reads::{read_discussion, read_discussions};
#[cfg(test)]
mod discussion_recovery_tests;

mod decision_input;
pub use decision_input::{
    DecideRecordReview, ReviewFeedback, SubmitRecordReview, WithdrawRecordReview,
};
mod automatic_submission;
mod decision;
mod decision_reads;
mod decision_state;

#[cfg(test)]
mod decision_tests;
