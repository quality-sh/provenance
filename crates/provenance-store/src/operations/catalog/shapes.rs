//! Shared execution shapes for catalog operations.

use super::{
    failures::ReadError, ContextKind, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
    PreparedRead, WireSchema,
};
use crate::{
    layout::ProvenanceLayout,
    state_store::StateStore,
    write_error::{SourceFailure, WriteError, WriteFailure},
};
use provenance_core::{protocol::failure::OperationFailure, ScopeId};
use serde::{de::DeserializeOwned, Serialize};

pub(super) trait OperationSpec: Send + Sync + 'static + Sized {
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

pub(super) trait OperationShape<S: OperationSpec> {
    type Failure: std::error::Error + Serialize + WireSchema + Send + 'static;

    const CONTEXT: ContextKind;

    fn failure_status(error: &Self::Failure) -> u16;
    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, Self::Failure>;
}

impl<S> Operation for S
where
    S: OperationSpec,
{
    type Request = S::Request;
    type Success = S::Success;
    type Failure = <S::Shape as OperationShape<S>>::Failure;

    const NAME: &'static str = S::NAME;
    const MUTATES: bool = S::MUTATES;
    const CONTEXT: ContextKind = <S::Shape as OperationShape<S>>::CONTEXT;
    const FAILURE_STATUSES: &'static [u16] = S::FAILURE_STATUSES;

    fn failure_status(error: &Self::Failure) -> u16 {
        S::Shape::failure_status(error)
    }

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

pub(super) struct GraphRead;

pub(super) trait GraphReadSpec: OperationSpec<Shape = GraphRead> {
    fn read(
        context: PreparedRead,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, ReadError>;
}

impl<S: GraphReadSpec> OperationShape<S> for GraphRead {
    type Failure = ReadError;

    const CONTEXT: ContextKind = ContextKind::Scoped;

    fn failure_status(error: &Self::Failure) -> u16 {
        error.status()
    }

    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, Self::Failure> {
        Box::pin(async move { S::read(context.graph()?, request).await })
    }
}

pub(super) struct ScopedRead;

pub(super) trait ScopedReadSpec: OperationSpec<Shape = ScopedRead> {
    fn read(
        store: &StateStore,
        scope: &ScopeId,
        request: Self::Request,
    ) -> anyhow::Result<Self::Success>;
}

impl<S: ScopedReadSpec> OperationShape<S> for ScopedRead {
    type Failure = ReadError;

    const CONTEXT: ContextKind = ContextKind::Scope;

    fn failure_status(error: &Self::Failure) -> u16 {
        error.status()
    }

    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, Self::Failure> {
        Box::pin(async move {
            let context = context.scope()?;
            let store = StateStore::new(ProvenanceLayout::new(context.root));
            S::read(&store, &context.scope, request).map_err(Into::into)
        })
    }
}

pub(super) struct ScopedWrite;

pub(super) trait ScopedWriteSpec: OperationSpec<Shape = ScopedWrite> {
    fn request_scope(_: &Self::Request) -> Option<&ScopeId> {
        None
    }

    fn write(
        store: &StateStore,
        selected_scope: ScopeId,
        request: Self::Request,
    ) -> anyhow::Result<Self::Success>;
}

impl<S: ScopedWriteSpec> OperationShape<S> for ScopedWrite {
    type Failure = WriteError;

    const CONTEXT: ContextKind = ContextKind::Scope;

    fn failure_status(error: &Self::Failure) -> u16 {
        error.status()
    }

    fn run(
        context: PreparedContext,
        request: S::Request,
    ) -> OperationFuture<S::Success, Self::Failure> {
        Box::pin(async move {
            let context = context.scope()?;
            if S::request_scope(&request).is_some_and(|scope| *scope != context.scope) {
                return Err(SourceFailure::wrap(
                    WriteFailure::ScopeMismatch,
                    anyhow::anyhow!("request scope does not match selected scope"),
                )
                .into());
            }
            let store = StateStore::new(ProvenanceLayout::new(context.root));
            S::write(&store, context.scope, request).map_err(Into::into)
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

macro_rules! scoped_write_operation {
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
    ($visibility:vis $name:ident, $wire:literal, $request:ty, $success:ty,
     $statuses:expr, $needs:expr,
     |$store:ident, $scope:ident, $input:ident| $body:expr) => {
        $crate::operations::catalog::shapes::scoped_write_operation!(
            @base $visibility $name, $wire, $request, $success, $statuses, $needs,
            |$store, $scope, $input| $body
        );
        impl $crate::operations::catalog::shapes::ScopedWriteSpec for $name {
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

pub(super) use {scoped_read_operation, scoped_write_operation};
