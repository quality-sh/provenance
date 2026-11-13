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

/// Creates the identity of a review record from the ID source of the store.
fn new_id(ids: &dyn crate::repository::IdSource) -> provenance_core::StableId {
    provenance_core::StableId::new(ids.new_uuid().to_string())
        .expect("UUID uses valid stable ID characters")
}

/// Refuses a review write to a scope that the manifest does not hold, with
/// the common `unknown_scope`.
fn ensure_manifest_scope(
    held: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
) -> anyhow::Result<()> {
    if held
        .manifest()?
        .scopes
        .iter()
        .any(|known| known.id == *scope)
    {
        return Ok(());
    }
    Err(crate::refusal::common(
        provenance_core::protocol::failure::OperationFailure::UnknownScope,
    ))
}

fn owner_matches(record: &impl serde::Serialize, owner: Option<&str>) -> anyhow::Result<()> {
    let value = serde_json::to_value(record)?;
    let saved_owner = value
        .get("declared_by")
        .or_else(|| value.get("made_by"))
        .and_then(serde_json::Value::as_str);
    if saved_owner != owner {
        return Err(crate::refusal::refuse!(
            (crate::write_error::WriteFailure::record_ownership_conflict(saved_owner)),
            "declared_by must match the existing owner"
        ));
    }
    Ok(())
}

// The file versions port and its contract build without host; the history
// reads in it and the review reads page through the projection on the host.
pub mod history;
#[cfg(feature = "host")]
mod reads;
#[cfg(feature = "host")]
pub use reads::{read_evidence, read_history};

#[cfg(test)]
#[cfg(feature = "host")]
mod concurrency_tests;
#[cfg(test)]
#[cfg(feature = "host")]
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
mod discussion_removal;
mod typed_adoption;
#[cfg(test)]
#[cfg(feature = "host")]
mod typed_adoption_tests;
pub use discussion_discovery::{read_discussion_conversation, read_discussion_list};
pub use discussion_messages::{read_discussion_message, read_discussion_messages};
pub use discussion_reads::{read_discussion, read_discussions};
#[cfg(test)]
#[cfg(feature = "host")]
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
pub(crate) use decision_reads::DocumentReviewState;
mod decision_state;

#[cfg(test)]
#[cfg(feature = "host")]
mod decision_tests;
#[cfg(test)]
#[cfg(feature = "host")]
mod write_permission_tests;
