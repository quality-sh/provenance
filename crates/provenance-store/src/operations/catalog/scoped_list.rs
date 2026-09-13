//! Shared adapter for complete native record lists in a selected scope.
macro_rules! scoped_list {
    ($name:ident, $wire:literal, $result:ident, $method:ident) => {
        pub struct $name;
        impl $crate::operations::catalog::Operation for $name {
            type Request = ();
            type Success = Vec<provenance_core::$result>;
            type Failure = $crate::operations::catalog::failures::ReadError;
            const NAME: &'static str = $wire;
            const CONTEXT: $crate::operations::catalog::ContextKind =
                $crate::operations::catalog::ContextKind::Scope;
            fn needs(_: &Self::Request) -> $crate::operations::catalog::ExecutionNeeds {
                &[$crate::operations::catalog::ExecutionNeed::GraphStorage]
            }
            fn failure_status(error: &$crate::operations::catalog::failures::ReadError) -> u16 {
                error.status()
            }
            fn run(
                context: $crate::operations::catalog::PreparedContext,
                _: Self::Request,
            ) -> $crate::operations::catalog::OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.scope()?;
                    Ok(
                        $crate::state_store::StateStore::new(
                            $crate::layout::ProvenanceLayout::new(context.root),
                        )
                        .$method(&context.scope)?,
                    )
                })
            }
        }
    };
}
pub(super) use scoped_list;
