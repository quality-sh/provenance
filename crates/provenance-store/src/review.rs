//! Guarded graph record edits and their review state.
mod classifier;
pub use classifier::{changed_fields, changes_revision, content_digest, review_revision};
pub(crate) mod guard;
mod input;
mod publication;
pub(crate) mod relationships;
// The resource reads serve the host use cases only.
#[cfg_attr(not(feature = "host"), allow(dead_code))]
mod resource_read;
pub(crate) use resource_read::RequirementResourceSnapshot;
mod save;
pub use input::{ListEdit, RequirementRelations, SaveRequirement};

/// Creates the identity of a review record.
fn new_id() -> provenance_core::StableId {
    provenance_core::StableId::new(uuid::Uuid::new_v4().to_string())
        .expect("UUID uses valid stable ID characters")
}

fn owner_matches(record: &impl serde::Serialize, owner: Option<&str>) -> anyhow::Result<()> {
    let value = serde_json::to_value(record)?;
    let saved_owner = value
        .get("declared_by")
        .or_else(|| value.get("made_by"))
        .and_then(serde_json::Value::as_str);
    if saved_owner != owner {
        return Err(crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::record_ownership_conflict(saved_owner),
            anyhow::anyhow!("declared_by must match the existing owner"),
        ));
    }
    Ok(())
}

// The review reads page through the projection; they run on the host.
#[cfg(feature = "host")]
mod history;
#[cfg(feature = "host")]
mod reads;
#[cfg(feature = "host")]
pub use reads::{read_evidence, read_history};

#[cfg(all(test, feature = "host"))]
mod concurrency_tests;
#[cfg(all(test, feature = "host"))]
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
#[cfg(feature = "host")]
mod discussion_discovery;
#[cfg(feature = "host")]
mod discussion_messages;
#[cfg(feature = "host")]
mod discussion_page;
#[cfg(feature = "host")]
mod discussion_reads;
mod discussion_removal;
mod typed_adoption;
#[cfg(all(test, feature = "host"))]
mod typed_adoption_tests;
#[cfg(feature = "host")]
pub use discussion_discovery::{read_discussion_conversation, read_discussion_list};
#[cfg(feature = "host")]
pub use discussion_messages::{read_discussion_message, read_discussion_messages};
#[cfg(feature = "host")]
pub use discussion_reads::{read_discussion, read_discussions};
#[cfg(all(test, feature = "host"))]
mod discussion_recovery_tests;

mod decision_input;
pub use decision_input::{
    DecideRecordReview, ReviewFeedback, SubmitRecordReview, WithdrawRecordReview,
};
mod automatic_submission;
mod cycle_index;
mod decision;
// The document review reads serve the host queries only.
#[cfg_attr(not(feature = "host"), allow(dead_code))]
mod decision_reads;
#[cfg(feature = "host")]
pub(crate) use decision_reads::DocumentReviewState;
mod decision_state;

#[cfg(all(test, feature = "host"))]
mod decision_tests;
#[cfg(all(test, feature = "host"))]
mod write_permission_tests;
