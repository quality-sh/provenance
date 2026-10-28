//! Public SDK operation functions.
//!
//! Every operation the wire adapter exposes is reachable here in
//! process. The CLI is a thin argv and stdio adapter over these
//! functions; nothing semantic lives in the adapter.

use anyhow::Context as _;
use std::str::FromStr as _;

use camino::Utf8Path;
use provenance_core::{EngineInfo, StableId, SDK_PROTOCOL_VERSION, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::rule;

use crate::repository::Repository;
use crate::state_store::{
    BeginVerificationInput, CompleteVerificationInput, TypedSpecInput, TypedSpecResult,
};

pub mod catalog;
pub mod files;
pub mod local_repository;
mod plan;
pub mod queries;
pub mod read_policy;
pub mod reader;
mod sites;
pub mod stamp;
pub use crate::write_error;

pub use local_repository::LocalRepository;
pub use plan::{AffectedRule, ReviewReason, RuleEvidence, TypedSpecPlan};

/// Advertises the engine contract. The engine version is the store's own.
pub fn engine_info(repository: &Repository) -> EngineInfo {
    EngineInfo {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: SDK_PROTOCOL_VERSION,
        state_schema_version: SUPPORTED_SCHEMA_VERSION.0,
        repository: repository.working_tree().to_path_buf(),
    }
}

/// Calculates one reconciliation, its affected Rules, and their evidence,
/// without writing.
pub fn plan(repository: &Repository, mut input: TypedSpecInput) -> anyhow::Result<TypedSpecPlan> {
    normalize_implementation_context(repository.working_tree(), &mut input)?;
    plan::typed_spec(repository, input)
}

/// Reconciles one language-owned desired-state document with canonical
/// state.
pub fn apply(
    repository: &Repository,
    mut input: TypedSpecInput,
) -> anyhow::Result<TypedSpecResult> {
    normalize_implementation_context(repository.working_tree(), &mut input)?;
    repository
        .store()
        .apply_typed_spec(repository.scope(), input)
}

/// Opens one verification run against a durable binding.
///
/// Parses the method word once, here, and carries the typed value through to
/// the store; the word reaches a stored row only through its `Display` form
/// at the run-row edge.
pub fn begin_verification(
    repository: &Repository,
    mut input: BeginVerificationInput,
) -> anyhow::Result<provenance_core::VerificationRun> {
    let method = provenance_scanner::Verification::from_str(&input.method).map_err(|error| {
        write_error::SourceFailure::wrap(
            write_error::WriteFailure::InvalidVerificationTarget,
            error,
        )
    })?;
    normalize_verification_context(repository.working_tree(), &mut input)?;
    repository
        .store()
        .begin_verification(repository.scope().clone(), input, method)
}

/// Completes one verification run as passed or failed.
pub fn complete_verification(
    repository: &Repository,
    input: CompleteVerificationInput,
) -> anyhow::Result<provenance_core::VerificationRun> {
    repository
        .store()
        .complete_verification(repository.scope(), input)
}

/// Lists verification runs, optionally for one Rule.
pub fn verification_runs(
    repository: &Repository,
    rule: Option<&StableId>,
) -> anyhow::Result<Vec<provenance_core::VerificationRun>> {
    Ok(repository
        .store()
        .list_verification_runs(repository.scope())?
        .into_iter()
        .filter(|run| rule.is_none_or(|rule| &run.rule_id == rule))
        .collect())
}

/// Lists verification bindings, optionally for one Rule.
pub fn verification_bindings(
    repository: &Repository,
    rule: Option<&StableId>,
) -> anyhow::Result<Vec<provenance_core::VerificationBinding>> {
    Ok(repository
        .store()
        .list_verification_bindings(repository.scope())?
        .into_iter()
        .filter(|binding| rule.is_none_or(|rule| &binding.rule_id == rule))
        .collect())
}

fn normalize_verification_context(
    repo: &Utf8Path,
    input: &mut BeginVerificationInput,
) -> anyhow::Result<()> {
    let file = input.file.as_ref().ok_or_else(|| {
        write_error::SourceFailure::wrap(
            write_error::WriteFailure::InvalidVerificationTarget,
            anyhow::anyhow!("file is required for a durable verification binding"),
        )
    })?;
    let relative = files::native_relative(repo, file)?;
    let root = files::RepositoryFiles::open(repo)?;
    let _opened = root.open_file(&relative)?;
    input.commit = clean_file_commit(repo, &relative);
    input.file = Some(relative);
    Ok(())
}

/// Resolves each implementation target to one repository file before any
/// store call; a target that does not resolve refuses the operation, so
/// apply writes nothing.
#[rule("rule_rust_store_resolves_implementation_symbols")]
#[rule("rule_rust_symbol_resolution_refuses_writes")]
fn normalize_implementation_context(
    repo: &Utf8Path,
    input: &mut TypedSpecInput,
) -> anyhow::Result<()> {
    for rule in &mut input.rules {
        let Some(implementation) = &mut rule.implementation else {
            continue;
        };
        let relative = files::native_relative(repo, &implementation.file).with_context(|| format!("implementation target `{}` is outside repository `{repo}` or is not a repository path", implementation.file))?;
        let root = files::RepositoryFiles::open(repo)?;
        let _opened = root.open_file(&relative).with_context(|| {
            format!(
                "implementation target `{}` does not exist or cannot be resolved",
                implementation.file
            )
        })?;
        implementation.file = relative;
    }
    Ok(())
}

fn clean_file_commit(repo: &Utf8Path, file: &Utf8Path) -> Option<String> {
    let tracked = crate::stale::git::command(repo)
        .args(["ls-files", "--error-unmatch", "--", file.as_str()])
        .current_dir(repo)
        .output()
        .ok()?;
    if !tracked.status.success() {
        return None;
    }
    let status = crate::stale::git::command(repo)
        .args(["status", "--porcelain", "--", file.as_str()])
        .current_dir(repo)
        .output()
        .ok()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return None;
    }
    let head = crate::stale::git::command(repo)
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .ok()?;
    if !head.status.success() {
        return None;
    }
    Some(String::from_utf8(head.stdout).ok()?.trim().to_string())
}

#[cfg(test)]
mod write_tests;
