//! Discussion lists return the complete native records in their stored order.
use super::{
    failures::ReadError, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use crate::{layout::ProvenanceLayout, state_store::StateStore};

macro_rules! list {
    ($name:ident, $wire:literal, $result:ident, $method:ident) => {
        pub struct $name;
        impl Operation for $name {
            type Request = ();
            type Success = Vec<provenance_core::$result>;
            type Failure = ReadError;
            const NAME: &'static str = $wire;
            const CONTEXT: super::ContextKind = super::ContextKind::Scope;
            fn needs(_: &Self::Request) -> ExecutionNeeds {
                &[ExecutionNeed::GraphStorage]
            }
            fn failure_status(error: &ReadError) -> u16 {
                error.status()
            }
            fn run(
                context: PreparedContext,
                _: Self::Request,
            ) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.scope()?;
                    Ok(StateStore::new(ProvenanceLayout::new(context.root))
                        .$method(&context.scope)?)
                })
            }
        }
    };
}
list!(ListThreads, "list-threads", Thread, list_threads);
list!(ListMessages, "list-messages", Message, list_messages);
