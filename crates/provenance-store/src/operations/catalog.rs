//! Typed operations shared by native and network callers.

mod actions;
mod authoring;
mod relationships;
pub use actions::*;
pub use relationships::*;
mod context;
mod creation;
mod discussion_discovery;
mod discussion_writes;
mod discussions;
mod drafts;
pub use drafts::*;
mod discussion_reads;
mod entry;
mod evidence;
mod failures;
mod ideation;
mod invoke;
mod records;
mod requirement_review;
pub(super) mod resource_lists;
pub(super) mod resource_members;
pub(super) mod resource_pages;
mod review_reads;
mod scoped_list;
mod shapes;
mod statement;
mod target_discussion_writes;
mod updates;
pub(super) mod verification_resources;
pub use discussion_discovery::*;
pub use discussion_reads::*;
pub use discussion_writes::*;
pub use requirement_review::*;
pub use resource_members::{GetReviewedResource, ReviewResource, ReviewedResourceRequest};
pub use review_reads::*;
pub use updates::*;

pub use authoring::{Apply, BeginVerification, CompleteVerification, Plan};
pub use context::{
    ContextKind, ContextResolver, ExecutionNeed, ExecutionNeeds, PreparedContext, PreparedRead,
    PreparedRepository, PreparedScope, RequestedContext,
};
pub use creation::{
    AddSourceReference, CreateRequirementNative, CreateResolution, CreateRule, CreateSource,
    PostThreadMessage,
};
pub use discussions::{ListMessages, ListThreads};
pub use entry::{Invoke, Operation, OperationFuture, WireSchema};
pub use ideation::{
    CreateAssertion, CreateDisposition, CreateProposal, ListAssertions, ListDispositions,
    ListProposals,
};
#[cfg(any(test, feature = "test-fixture"))]
pub use invoke::Direct;
pub use invoke::{frame_failure, invoke_authorized_typed, invoke_typed};
pub use statement::CheckStatement;
pub use target_discussion_writes::{WriteTargetDiscussion, WriteTargetDiscussionRequest};

#[cfg(test)]
mod tests;

pub use records::{Get, Info, Neighbors, ReadDocument, ResolveRecord, Search, Trace};

pub use evidence::{
    Evidence, Impact, ResolveSymbol, Stale, VerificationBindings, VerificationRuns,
};
