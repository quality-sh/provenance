//! Verification orchestration over the store's begin and complete
//! operations.

use std::panic::{catch_unwind, AssertUnwindSafe};

use provenance_core::authoring::RuleHandle;
use provenance_core::DeclarationAddress;
use provenance_macros::rule;
use provenance_store::operations;
use provenance_store::state_store::{
    BeginVerificationInput, CompleteVerificationInput, DeclarationReferenceInput,
};

use crate::Settings;

/// Which Rule a verification run cites: a canonical rule id, or the
/// declaration address of a rule this spec owner declared.
pub enum VerifyTarget<'a> {
    Rule(&'a str),
    Declaration(&'a DeclarationAddress),
}

impl<'a> From<&'a str> for VerifyTarget<'a> {
    fn from(rule: &'a str) -> Self {
        Self::Rule(rule)
    }
}

impl<'a> From<&'a String> for VerifyTarget<'a> {
    fn from(rule: &'a String) -> Self {
        Self::Rule(rule)
    }
}

impl<'a> From<&'a RuleHandle> for VerifyTarget<'a> {
    fn from(handle: &'a RuleHandle) -> Self {
        Self::Declaration(&handle.address)
    }
}

/// Runs one verification callback and records its outcome.
///
/// The callback runs under `catch_unwind`. An `Err` or an unwind first
/// records the run as failed with a serialized payload, then propagates;
/// a completion failure stays subordinate to the callback failure. With
/// `panic = "abort"` an unwind never reaches the recorder. The call
/// site's file becomes the binding file, so call this from the test that
/// carries the evidence.
#[track_caller]
#[rule("rule_rust_verification_records_failure_first")]
#[rule("rule_rust_sdk_facade_delegates_semantics")]
pub fn verify<'a, E: std::fmt::Display>(
    target: impl Into<VerifyTarget<'a>>,
    key: &str,
    callback: impl FnOnce() -> Result<(), E>,
) -> anyhow::Result<()> {
    let file = std::panic::Location::caller().file().to_string();
    let settings = Settings::from_env();
    let scope = settings.scope_id()?;
    let (rule, declaration) = match target.into() {
        VerifyTarget::Rule(rule) => (Some(rule.to_string()), None),
        VerifyTarget::Declaration(address) => (
            None,
            Some(DeclarationReferenceInput {
                declared_by: settings.owner.clone(),
                address: address.clone(),
            }),
        ),
    };
    let run = operations::begin_verification(
        settings.repository.clone(),
        scope.clone(),
        BeginVerificationInput {
            rule,
            declaration,
            key: key.to_string(),
            method: "examples".to_string(),
            declared_by: settings.verification_owner.clone(),
            file: Some(file.into()),
            symbol: None,
            commit: None,
        },
    )?;
    let complete = |status: &str, error: Option<String>| {
        operations::complete_verification(
            settings.repository.clone(),
            &scope,
            CompleteVerificationInput {
                run: run.id.as_str().to_string(),
                status: status.to_string(),
                error,
            },
        )
    };
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(Ok(())) => {
            complete("passed", None)?;
            Ok(())
        }
        Ok(Err(error)) => {
            let payload = error.to_string();
            // The callback failure stays the primary failure.
            let _ = complete("failed", Some(payload.clone()));
            Err(anyhow::anyhow!(payload))
        }
        Err(panic) => {
            let _ = complete("failed", Some(panic_payload(panic.as_ref())));
            std::panic::resume_unwind(panic);
        }
    }
}

fn panic_payload(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|text| (*text).to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_string())
}
