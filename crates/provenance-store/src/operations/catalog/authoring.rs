//! Authoring and verification use the native Store implementation.
use super::{shapes::scoped_command_operation, ExecutionNeed};
use crate::state_store::{
    BeginVerificationInput, CompleteVerificationInput, TypedSpecInput, TypedSpecResult,
};
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};

fn external_file(file: &camino::Utf8Path, field: &str) -> Result<(), OperationFailure> {
    crate::operations::files::validate_relative(file).map_err(|_| OperationFailure::InvalidInput {
        field: Some(field.into()),
        reason: InvalidInputReason::InvalidValue,
    })
}
fn spec_files(input: &TypedSpecInput) -> Result<(), OperationFailure> {
    for (index, rule) in input.rules.iter().enumerate() {
        if let Some(implementation) = &rule.implementation {
            external_file(
                &implementation.file,
                &format!("request.rules[{index}].implementation.file"),
            )?;
        }
    }
    Ok(())
}
fn verification_file(input: &BeginVerificationInput) -> Result<(), OperationFailure> {
    if let Some(file) = &input.file {
        external_file(file, "request.file")?;
    }
    Ok(())
}

macro_rules! operation {
    ($name:ident, $wire:literal, $request:ty, $success:ty, $mutates:literal, $validate:expr, $handler:expr, [$($need:ident),*]) => {
        scoped_command_operation!(
            pub $name, $wire, $request, $success, mutates = $mutates, &[409],
            &[$(ExecutionNeed::$need),*], scope = none, validate = $validate,
            |context, request| ($handler)(context, request)
        );
    };
}
operation!(
    Plan,
    "plan",
    TypedSpecInput,
    crate::operations::TypedSpecPlan,
    false,
    spec_files,
    |context: super::PreparedScope, input| crate::operations::plan(
        Some(context.root),
        &context.scope,
        input
    ),
    [GraphStorage, RepositoryFiles, Dictionary]
);
operation!(
    Apply,
    "apply",
    TypedSpecInput,
    TypedSpecResult,
    true,
    spec_files,
    |context: super::PreparedScope, input| crate::operations::apply(
        Some(context.root),
        &context.scope,
        input
    ),
    [GraphStorage, RepositoryFiles, Dictionary]
);
operation!(
    BeginVerification,
    "begin-verification",
    BeginVerificationInput,
    provenance_core::VerificationRun,
    true,
    verification_file,
    |context: super::PreparedScope, input| crate::operations::begin_verification(
        Some(context.root),
        context.scope,
        input
    ),
    [GraphStorage, RepositoryFiles, Git, RunStorage]
);
operation!(
    CompleteVerification,
    "complete-verification",
    CompleteVerificationInput,
    provenance_core::VerificationRun,
    true,
    |_: &CompleteVerificationInput| Ok(()),
    |context: super::PreparedScope, input| crate::operations::complete_verification(
        Some(context.root),
        &context.scope,
        input
    ),
    [GraphStorage, RunStorage]
);
