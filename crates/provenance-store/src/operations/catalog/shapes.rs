//! Shared execution shapes for catalog operations.

use super::{
    failures::ReadError, ContextKind, ExecutionNeeds, HandlerFailure, Operation, OperationFuture,
    PreparedContext, PreparedRead, WireSchema,
};
use crate::{state_store::StateStore, write_error::WriteError};
use provenance_core::{
    protocol::failure::{OperationError, OperationFailure},
    ScopeId,
};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;

pub trait OperationSpec: Send + Sync + 'static + Sized {
    type Request: DeserializeOwned + WireSchema + Send + 'static;
    type Success: Serialize + WireSchema + Send + 'static;
    type Shape: OperationShape<Self>;

    const NAME: &'static str;
    const MUTATES: bool = false;
    const FAILURE_STATUSES: &'static [u16];

    fn needs(request: &Self::Request) -> ExecutionNeeds;

    fn validate_external(_: &Self::Request) -> Result<(), OperationFailure> {
        Ok(())
    }
}

pub trait OperationShape<S: OperationSpec> {
    type Failure: HandlerFailure;

    const CONTEXT: ContextKind;

    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, OperationError<Self::Failure>>;
}

/// Runs one handler and gives its error as the failure of the handler.
pub(super) fn handled<T: 'static, F: 'static>(
    handler: impl Future<Output = Result<T, F>> + Send + 'static,
) -> OperationFuture<T, OperationError<F>> {
    Box::pin(async move { handler.await.map_err(OperationError::Handler) })
}

impl<S> Operation for S
where
    S: OperationSpec,
{
    type Request = S::Request;
    type Success = S::Success;
    type Failure = OperationError<<S::Shape as OperationShape<S>>::Failure>;

    const NAME: &'static str = S::NAME;
    const MUTATES: bool = S::MUTATES;
    const CONTEXT: ContextKind = <S::Shape as OperationShape<S>>::CONTEXT;
    const FAILURE_STATUSES: &'static [u16] = S::FAILURE_STATUSES;

    fn validate_external(request: &Self::Request) -> Result<(), OperationFailure> {
        S::validate_external(request)
    }

    fn needs(request: &Self::Request) -> ExecutionNeeds {
        S::needs(request)
    }

    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        S::Shape::run(context, request)
    }
}

pub struct GraphRead;

pub trait GraphReadSpec: OperationSpec<Shape = GraphRead> {
    fn read(
        context: PreparedRead,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, ReadError>;
}

impl<S: GraphReadSpec> OperationShape<S> for GraphRead {
    type Failure = ReadError;

    const CONTEXT: ContextKind = ContextKind::Scoped;

    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, OperationError<Self::Failure>> {
        handled(async move {
            let context = context.graph()?;
            let root = context.repository.layout().root().to_owned();
            S::read(context, request)
                .await
                .map_err(|error| error.in_repository(&root))
        })
    }
}

pub struct ScopedRead;

pub trait ScopedReadSpec: OperationSpec<Shape = ScopedRead> {
    fn read(
        store: &StateStore,
        scope: &ScopeId,
        request: Self::Request,
    ) -> anyhow::Result<Self::Success>;
}

impl<S: ScopedReadSpec> OperationShape<S> for ScopedRead {
    type Failure = ReadError;

    const CONTEXT: ContextKind = ContextKind::Scope;

    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, OperationError<Self::Failure>> {
        handled(async move {
            let context = context.scope()?;
            let store = context.repository.store();
            S::read(&store, context.repository.scope(), request)
                .map_err(|error| ReadError(error).in_repository(context.repository.layout().root()))
        })
    }
}

pub struct ScopedWrite;

pub trait ScopedWriteSpec: OperationSpec<Shape = ScopedWrite> {
    fn request_scope(request: &Self::Request) -> Option<&ScopeId>;

    fn write(
        store: &StateStore,
        selected_scope: ScopeId,
        request: Self::Request,
    ) -> anyhow::Result<Self::Success>;
}

impl<S: ScopedWriteSpec> OperationShape<S> for ScopedWrite {
    type Failure = WriteError;

    const CONTEXT: ContextKind = ContextKind::Scope;

    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, OperationError<Self::Failure>> {
        handled(async move {
            let context = context.scope()?;
            if let Some(scope) = S::request_scope(&request) {
                context.repository.ensure_selected(scope)?;
            }
            let store = context.repository.store();
            S::write(&store, context.repository.scope().clone(), request).map_err(|error| {
                WriteError(error).in_repository(context.repository.layout().root())
            })
        })
    }
}

pub struct ScopedCommand;

pub trait ScopedCommandSpec: OperationSpec<Shape = ScopedCommand> {
    fn request_scope(request: &Self::Request) -> Option<&ScopeId>;

    fn execute(
        context: super::PreparedScope,
        request: Self::Request,
    ) -> anyhow::Result<Self::Success>;
}

impl<S: ScopedCommandSpec> OperationShape<S> for ScopedCommand {
    type Failure = WriteError;

    const CONTEXT: ContextKind = ContextKind::Scope;

    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, OperationError<Self::Failure>> {
        handled(async move {
            let context = context.scope()?;
            if let Some(scope) = S::request_scope(&request) {
                context.repository.ensure_selected(scope)?;
            }
            let root = context.repository.layout().root().to_owned();
            S::execute(context, request).map_err(|error| WriteError(error).in_repository(&root))
        })
    }
}

macro_rules! scoped_read_operation {
    ($visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $statuses:expr, $needs:expr, |$store:ident, $scope:ident, $input:ident| $body:expr) => {
        $visibility struct $name;
        impl $crate::operations::catalog::shapes::OperationSpec for $name {
            type Request = $request;
            type Success = $success;
            type Shape = $crate::operations::catalog::shapes::ScopedRead;
            const NAME: &'static str = $wire;
            const FAILURE_STATUSES: &'static [u16] = $statuses;
            fn needs(_: &Self::Request) -> $crate::operations::catalog::ExecutionNeeds {
                $needs
            }
        }
        impl $crate::operations::catalog::shapes::ScopedReadSpec for $name {
            fn read(
                $store: &$crate::state_store::StateStore,
                $scope: &provenance_core::ScopeId,
                $input: Self::Request,
            ) -> anyhow::Result<Self::Success> {
                $body
            }
        }
    };
}

macro_rules! graph_read_operation {
    ($visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $statuses:expr, $needs:expr,
     |$context:ident, $input:ident| $body:expr) => {
        $crate::operations::catalog::shapes::graph_read_operation!(
            @base $visibility $name, $wire, $request, $success, $statuses, $needs, []
        );
        impl $crate::operations::catalog::shapes::GraphReadSpec for $name {
            fn read(
                $context: $crate::operations::catalog::PreparedRead,
                $input: Self::Request,
            ) -> $crate::operations::catalog::OperationFuture<
                Self::Success,
                $crate::operations::catalog::failures::ReadError,
            > {
                Box::pin($body)
            }
        }
    };
    ($visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $statuses:expr, $needs:expr, validate = $validate:expr,
     |$context:ident, $input:ident| $body:expr) => {
        $crate::operations::catalog::shapes::graph_read_operation!(
            @base $visibility $name, $wire, $request, $success, $statuses, $needs, [
            fn validate_external(
                request: &Self::Request,
            ) -> Result<(), provenance_core::protocol::failure::OperationFailure> {
                ($validate)(request)
            }
            ]
        );
        impl $crate::operations::catalog::shapes::GraphReadSpec for $name {
            fn read(
                $context: $crate::operations::catalog::PreparedRead,
                $input: Self::Request,
            ) -> $crate::operations::catalog::OperationFuture<
                Self::Success,
                $crate::operations::catalog::failures::ReadError,
            > {
                Box::pin($body)
            }
        }
    };
    (@base $visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $statuses:expr, $needs:expr, [$($extra:item)*]) => {
        $visibility struct $name;
        impl $crate::operations::catalog::shapes::OperationSpec for $name {
            type Request = $request;
            type Success = $success;
            type Shape = $crate::operations::catalog::shapes::GraphRead;
            const NAME: &'static str = $wire;
            const FAILURE_STATUSES: &'static [u16] = $statuses;
            fn needs(request: &Self::Request) -> $crate::operations::catalog::ExecutionNeeds {
                ($needs)(request)
            }
            $($extra)*
        }
    };
}

macro_rules! scoped_write_operation {
    ($visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $statuses:expr, $needs:expr, scope = none,
     |$store:ident, $scope:ident, $input:ident| $body:expr) => {
        $crate::operations::catalog::shapes::scoped_write_operation!(
            @base $visibility $name, $wire, $request, $success, $statuses, $needs,
            |$store, $scope, $input| $body
        );
        impl $crate::operations::catalog::shapes::ScopedWriteSpec for $name {
            fn request_scope(_: &Self::Request) -> Option<&provenance_core::ScopeId> {
                None
            }
            fn write(
                $store: &$crate::state_store::StateStore,
                $scope: provenance_core::ScopeId,
                $input: Self::Request,
            ) -> anyhow::Result<Self::Success> {
                $body
            }
        }
    };
    ($visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $statuses:expr, $needs:expr, scope = $scope_field:ident,
     |$store:ident, $scope:ident, $input:ident| $body:expr) => {
        $crate::operations::catalog::shapes::scoped_write_operation!(
            @base $visibility $name, $wire, $request, $success, $statuses, $needs,
            |$store, $scope, $input| $body
        );
        impl $crate::operations::catalog::shapes::ScopedWriteSpec for $name {
            fn request_scope(request: &Self::Request) -> Option<&provenance_core::ScopeId> {
                Some(&request.$scope_field)
            }
            fn write(
                $store: &$crate::state_store::StateStore,
                $scope: provenance_core::ScopeId,
                $input: Self::Request,
            ) -> anyhow::Result<Self::Success> {
                $body
            }
        }
    };
    (@base $visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $statuses:expr, $needs:expr,
     |$store:ident, $scope:ident, $input:ident| $body:expr) => {
        $visibility struct $name;
        impl $crate::operations::catalog::shapes::OperationSpec for $name {
            type Request = $request;
            type Success = $success;
            type Shape = $crate::operations::catalog::shapes::ScopedWrite;
            const NAME: &'static str = $wire;
            const MUTATES: bool = true;
            const FAILURE_STATUSES: &'static [u16] = $statuses;
            fn needs(_: &Self::Request) -> $crate::operations::catalog::ExecutionNeeds {
                $needs
            }
        }
    };
}

macro_rules! scoped_command_operation {
    ($visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     mutates = $mutates:literal, $statuses:expr, $needs:expr, scope = none,
     validate = $validate:expr,
     |$context:ident, $input:ident| $body:expr) => {
        $crate::operations::catalog::shapes::scoped_command_operation!(
            @base $visibility $name, $wire, $request, $success, $mutates, $statuses, $needs,
            $validate
        );
        impl $crate::operations::catalog::shapes::ScopedCommandSpec for $name {
            fn request_scope(_: &Self::Request) -> Option<&provenance_core::ScopeId> {
                None
            }
            fn execute(
                $context: $crate::operations::catalog::PreparedScope,
                $input: Self::Request,
            ) -> anyhow::Result<Self::Success> {
                $body
            }
        }
    };
    ($visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     mutates = $mutates:literal, $statuses:expr, $needs:expr, scope = $scope_field:ident,
     validate = $validate:expr,
     |$context:ident, $input:ident| $body:expr) => {
        $crate::operations::catalog::shapes::scoped_command_operation!(
            @base $visibility $name, $wire, $request, $success, $mutates, $statuses, $needs,
            $validate
        );
        impl $crate::operations::catalog::shapes::ScopedCommandSpec for $name {
            fn request_scope(request: &Self::Request) -> Option<&provenance_core::ScopeId> {
                Some(&request.$scope_field)
            }
            fn execute(
                $context: $crate::operations::catalog::PreparedScope,
                $input: Self::Request,
            ) -> anyhow::Result<Self::Success> {
                $body
            }
        }
    };
    (@base $visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $mutates:literal, $statuses:expr, $needs:expr, $validate:expr) => {
        $visibility struct $name;
        impl $crate::operations::catalog::shapes::OperationSpec for $name {
            type Request = $request;
            type Success = $success;
            type Shape = $crate::operations::catalog::shapes::ScopedCommand;
            const NAME: &'static str = $wire;
            const MUTATES: bool = $mutates;
            const FAILURE_STATUSES: &'static [u16] = $statuses;
            fn needs(_: &Self::Request) -> $crate::operations::catalog::ExecutionNeeds {
                $needs
            }
            fn validate_external(
                request: &Self::Request,
            ) -> Result<(), provenance_core::protocol::failure::OperationFailure> {
                ($validate)(request)
            }
        }
    };
}

pub(super) use {
    graph_read_operation, scoped_command_operation, scoped_read_operation, scoped_write_operation,
};
