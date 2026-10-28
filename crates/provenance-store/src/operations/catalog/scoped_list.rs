//! Shared adapter for complete native record lists in a selected scope.
macro_rules! scoped_list {
    ($name:ident, $wire:literal, $result:ident, $method:ident) => {
        $crate::operations::catalog::shapes::scoped_read_operation!(
            pub $name,
            $wire,
            (),
            Vec<provenance_core::$result>,
            &[409],
            &[$crate::operations::catalog::ExecutionNeed::GraphStorage],
            |store, scope, _request| store.$method(scope)
        );
    };
    ($name:ident, $wire:literal, type $result:ty, $method:ident) => {
        $crate::operations::catalog::shapes::scoped_read_operation!(
            pub $name,
            $wire,
            (),
            Vec<$result>,
            &[409],
            &[$crate::operations::catalog::ExecutionNeed::GraphStorage],
            |store, scope, _request| store.$method(scope)
        );
    };
}
pub(super) use scoped_list;
